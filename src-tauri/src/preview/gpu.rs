// GPU-accelerated frame processing using WGPU compute shaders.
//
// Provides colour-space conversion (NV12/YUY2/BGR -> RGB) on the GPU,
// falling back to CPU when no suitable adapter is available.
//
// Shaders output RGBA (1 u32 per pixel) to avoid byte-alignment issues
// with WGSL storage buffers. The alpha channel is stripped on readback.

use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use parking_lot::RwLock;
use tracing::{info, warn};

/// Information about a GPU adapter available on the system.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuAdapterInfo {
    /// Index in the adapter list (used for selection).
    pub index: usize,
    /// Human-readable adapter name (e.g. "NVIDIA GeForce RTX 3080").
    pub name: String,
    /// Graphics backend (e.g. "Vulkan", "Dx12", "Metal").
    pub backend: String,
    /// Device type (e.g. "DiscreteGpu", "IntegratedGpu", "Cpu").
    pub device_type: String,
}

/// Uniform parameters passed to compute shaders via a uniform buffer.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ConvertParams {
    width: u32,
    height: u32,
    _pad0: u32,
    _pad1: u32,
}

/// Holds the WGPU device, queue, and pre-compiled pipelines for colour conversion.
///
/// Shared across all camera sessions via `Arc`. Thread-safe (Device and Queue
/// are `Send + Sync` by design).
pub struct GpuContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
    adapter_info: wgpu::AdapterInfo,
    nv12_pipeline: wgpu::ComputePipeline,
    yuy2_pipeline: wgpu::ComputePipeline,
    bgr_pipeline: wgpu::ComputePipeline,
}

/// WGSL compute shader for NV12 -> RGBA conversion.
///
/// NV12 is a 4:2:0 planar format: full-resolution Y plane followed by
/// interleaved UV at half resolution in both dimensions. Uses BT.601
/// coefficients matching the existing CPU path.
///
/// Output is RGBA (1 u32 per pixel) to avoid byte-alignment issues.
const NV12_SHADER: &str = r#"
struct Params {
    width: u32,
    height: u32,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> input: array<u32>;
@group(0) @binding(2) var<storage, read_write> output: array<u32>;

fn read_byte(offset: u32) -> u32 {
    let word_idx = offset / 4u;
    let byte_idx = offset % 4u;
    return (input[word_idx] >> (byte_idx * 8u)) & 0xFFu;
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let col = gid.x;
    let row = gid.y;
    if col >= params.width || row >= params.height {
        return;
    }

    let y_val = i32(read_byte(row * params.width + col));
    let uv_offset = params.width * params.height;
    let uv_row = row / 2u;
    let uv_col = (col / 2u) * 2u;
    let u_val = i32(read_byte(uv_offset + uv_row * params.width + uv_col)) - 128;
    let v_val = i32(read_byte(uv_offset + uv_row * params.width + uv_col + 1u)) - 128;

    let r = u32(clamp((y_val * 256 + 359 * v_val) >> 8, 0, 255));
    let g = u32(clamp((y_val * 256 - 88 * u_val - 183 * v_val) >> 8, 0, 255));
    let b = u32(clamp((y_val * 256 + 454 * u_val) >> 8, 0, 255));

    let pixel_idx = row * params.width + col;
    output[pixel_idx] = r | (g << 8u) | (b << 16u) | (255u << 24u);
}
"#;

/// WGSL compute shader for YUY2 -> RGBA conversion.
///
/// YUY2 packs two pixels per 4-byte macro-pixel: [Y0, U, Y1, V].
/// Each invocation processes one macro-pixel (two output pixels).
const YUY2_SHADER: &str = r#"
struct Params {
    width: u32,
    height: u32,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> input: array<u32>;
@group(0) @binding(2) var<storage, read_write> output: array<u32>;

fn read_byte(offset: u32) -> u32 {
    let word_idx = offset / 4u;
    let byte_idx = offset % 4u;
    return (input[word_idx] >> (byte_idx * 8u)) & 0xFFu;
}

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let macro_idx = gid.x;
    let total_macros = (params.width * params.height) / 2u;
    if macro_idx >= total_macros {
        return;
    }

    let src = macro_idx * 4u;
    let y0 = i32(read_byte(src));
    let u_val = i32(read_byte(src + 1u)) - 128;
    let y1 = i32(read_byte(src + 2u));
    let v_val = i32(read_byte(src + 3u)) - 128;

    let r0 = u32(clamp((y0 * 256 + 359 * v_val) >> 8, 0, 255));
    let g0 = u32(clamp((y0 * 256 - 88 * u_val - 183 * v_val) >> 8, 0, 255));
    let b0 = u32(clamp((y0 * 256 + 454 * u_val) >> 8, 0, 255));

    let r1 = u32(clamp((y1 * 256 + 359 * v_val) >> 8, 0, 255));
    let g1 = u32(clamp((y1 * 256 - 88 * u_val - 183 * v_val) >> 8, 0, 255));
    let b1 = u32(clamp((y1 * 256 + 454 * u_val) >> 8, 0, 255));

    let out_idx = macro_idx * 2u;
    output[out_idx] = r0 | (g0 << 8u) | (b0 << 16u) | (255u << 24u);
    output[out_idx + 1u] = r1 | (g1 << 8u) | (b1 << 16u) | (255u << 24u);
}
"#;

/// WGSL compute shader for BGR24 bottom-up -> RGBA top-down.
///
/// Flips rows vertically and swaps blue/red channels.
const BGR_SHADER: &str = r#"
struct Params {
    width: u32,
    height: u32,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> input: array<u32>;
@group(0) @binding(2) var<storage, read_write> output: array<u32>;

fn read_byte(offset: u32) -> u32 {
    let word_idx = offset / 4u;
    let byte_idx = offset % 4u;
    return (input[word_idx] >> (byte_idx * 8u)) & 0xFFu;
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let col = gid.x;
    let row = gid.y;
    if col >= params.width || row >= params.height {
        return;
    }

    let stride = params.width * 3u;
    let src_row = (params.height - 1u - row) * stride;
    let src_offset = src_row + col * 3u;

    let b = read_byte(src_offset);
    let g = read_byte(src_offset + 1u);
    let r = read_byte(src_offset + 2u);

    let pixel_idx = row * params.width + col;
    output[pixel_idx] = r | (g << 8u) | (b << 16u) | (255u << 24u);
}
"#;

impl GpuContext {
    /// Create a new GPU context, optionally selecting a specific adapter by index.
    ///
    /// Returns `None` if no suitable adapter is found or initialisation fails.
    pub fn new(adapter_index: Option<usize>) -> Option<Self> {
        pollster::block_on(Self::new_async(adapter_index))
    }

    /// Async initialisation — enumerate adapters, request device, compile pipelines.
    async fn new_async(adapter_index: Option<usize>) -> Option<Self> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapters: Vec<wgpu::Adapter> = instance.enumerate_adapters(wgpu::Backends::all()).await;
        if adapters.is_empty() {
            warn!("No GPU adapters found");
            return None;
        }

        let adapter = if let Some(idx) = adapter_index {
            adapters.into_iter().nth(idx)?
        } else {
            // Prefer discrete GPU, then integrated, then anything
            let mut adapters = adapters;
            adapters.sort_by_key(|a| match a.get_info().device_type {
                wgpu::DeviceType::DiscreteGpu => 0,
                wgpu::DeviceType::IntegratedGpu => 1,
                wgpu::DeviceType::VirtualGpu => 2,
                wgpu::DeviceType::Cpu => 3,
                wgpu::DeviceType::Other => 4,
            });
            adapters.into_iter().next()?
        };

        let adapter_info = adapter.get_info();
        info!(
            "Selected GPU adapter: {} ({:?}, {:?})",
            adapter_info.name, adapter_info.backend, adapter_info.device_type
        );

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("cameras-gpu"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults(),
                ..Default::default()
            })
            .await
            .ok()?;

        let nv12_pipeline = Self::create_pipeline(&device, "nv12_to_rgb", NV12_SHADER);
        let yuy2_pipeline = Self::create_pipeline(&device, "yuy2_to_rgb", YUY2_SHADER);
        let bgr_pipeline = Self::create_pipeline(&device, "bgr_to_rgb", BGR_SHADER);

        Some(Self {
            device,
            queue,
            adapter_info,
            nv12_pipeline,
            yuy2_pipeline,
            bgr_pipeline,
        })
    }

    /// Compile a compute pipeline from WGSL source.
    fn create_pipeline(device: &wgpu::Device, label: &str, source: &str) -> wgpu::ComputePipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(label),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });

        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some(label),
            layout: None,
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        })
    }

    /// Enumerate all available GPU adapters on the system.
    pub fn enumerate_adapters() -> Vec<GpuAdapterInfo> {
        pollster::block_on(Self::enumerate_adapters_async())
    }

    /// Async adapter enumeration.
    async fn enumerate_adapters_async() -> Vec<GpuAdapterInfo> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapters = instance.enumerate_adapters(wgpu::Backends::all()).await;

        adapters
            .into_iter()
            .enumerate()
            .map(|(index, adapter)| {
                let info = adapter.get_info();
                GpuAdapterInfo {
                    index,
                    name: info.name,
                    backend: format!("{:?}", info.backend),
                    device_type: format!("{:?}", info.device_type),
                }
            })
            .collect()
    }

    /// Return information about the currently active adapter.
    pub fn adapter_info(&self) -> GpuAdapterInfo {
        GpuAdapterInfo {
            index: 0, // current adapter
            name: self.adapter_info.name.clone(),
            backend: format!("{:?}", self.adapter_info.backend),
            device_type: format!("{:?}", self.adapter_info.device_type),
        }
    }

    /// Convert NV12 frame data to RGB24 using the GPU.
    pub fn convert_nv12_to_rgb(&self, data: &[u8], width: usize, height: usize) -> Option<Vec<u8>> {
        self.run_conversion(&self.nv12_pipeline, data, width, height)
    }

    /// Convert YUY2 frame data to RGB24 using the GPU.
    pub fn convert_yuy2_to_rgb(&self, data: &[u8], width: usize, height: usize) -> Option<Vec<u8>> {
        self.run_conversion(&self.yuy2_pipeline, data, width, height)
    }

    /// Convert BGR24 bottom-up frame data to RGB24 top-down using the GPU.
    pub fn convert_bgr_to_rgb(&self, data: &[u8], width: usize, height: usize) -> Option<Vec<u8>> {
        self.run_conversion(&self.bgr_pipeline, data, width, height)
    }

    /// Run a colour conversion compute shader.
    ///
    /// The shader outputs RGBA (1 u32 per pixel). This method strips the
    /// alpha channel on readback, returning RGB24 data.
    fn run_conversion(
        &self,
        pipeline: &wgpu::ComputePipeline,
        input_data: &[u8],
        width: usize,
        height: usize,
    ) -> Option<Vec<u8>> {
        use wgpu::util::DeviceExt;

        let pixel_count = width * height;

        let params = ConvertParams {
            width: width as u32,
            height: height as u32,
            _pad0: 0,
            _pad1: 0,
        };

        let uniform_buf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("params"),
                contents: bytemuck::bytes_of(&params),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        // Pad input to 4-byte alignment for u32 array access in shaders
        let padded_input = pad_to_alignment(input_data, 4);
        let input_buf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("input"),
                contents: &padded_input,
                usage: wgpu::BufferUsages::STORAGE,
            });

        // Output buffer: 1 u32 (4 bytes) per pixel for RGBA
        let output_byte_size = (pixel_count * 4) as u64;
        let output_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("output"),
            size: output_byte_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let staging_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("staging"),
            size: output_byte_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = pipeline.get_bind_group_layout(0);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("convert"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: input_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: output_buf.as_entire_binding(),
                },
            ],
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("convert"),
            });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("convert"),
                timestamp_writes: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &bind_group, &[]);

            // YUY2 uses 1D workgroups (256), NV12 and BGR use 2D (16x16)
            let is_yuy2 = std::ptr::eq(pipeline, &self.yuy2_pipeline);
            if is_yuy2 {
                let total_macros = pixel_count / 2;
                let workgroups_x = (total_macros as u32).div_ceil(256);
                pass.dispatch_workgroups(workgroups_x, 1, 1);
            } else {
                let workgroups_x = (width as u32).div_ceil(16);
                let workgroups_y = (height as u32).div_ceil(16);
                pass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
            }
        }

        encoder.copy_buffer_to_buffer(&output_buf, 0, &staging_buf, 0, output_byte_size);
        self.queue.submit(std::iter::once(encoder.finish()));

        // Block until the GPU work completes and map the staging buffer
        let buffer_slice = staging_buf.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());

        receiver.recv().ok()?.ok()?;
        let mapped = buffer_slice.get_mapped_range();

        // Convert RGBA -> RGB by stripping the alpha channel
        let rgba = &mapped[..];
        let mut rgb = Vec::with_capacity(pixel_count * 3);
        for pixel in rgba.chunks_exact(4) {
            rgb.push(pixel[0]); // R
            rgb.push(pixel[1]); // G
            rgb.push(pixel[2]); // B
        }

        drop(mapped);
        staging_buf.unmap();

        Some(rgb)
    }
}

/// Pad a byte slice to the given alignment by appending zero bytes.
fn pad_to_alignment(data: &[u8], alignment: usize) -> Vec<u8> {
    let padded_len = align_up(data.len(), alignment);
    let mut padded = Vec::with_capacity(padded_len);
    padded.extend_from_slice(data);
    padded.resize(padded_len, 0);
    padded
}

/// Round up to the nearest multiple of `alignment`.
fn align_up(value: usize, alignment: usize) -> usize {
    (value + alignment - 1) & !(alignment - 1)
}

/// Thread-safe wrapper for the optional GPU context.
///
/// Allows switching adapters at runtime by swapping the inner context
/// behind an `RwLock`. Readers (frame conversion) take a read lock;
/// adapter switching takes a write lock.
pub struct GpuState {
    context: RwLock<Option<Arc<GpuContext>>>,
}

impl GpuState {
    /// Initialise GPU state, attempting to create a context with the default adapter.
    pub fn new() -> Self {
        let context = match GpuContext::new(None) {
            Some(ctx) => {
                info!("GPU acceleration enabled: {}", ctx.adapter_info.name);
                Some(Arc::new(ctx))
            }
            None => {
                warn!("GPU acceleration unavailable — falling back to CPU");
                None
            }
        };
        Self {
            context: RwLock::new(context),
        }
    }

    /// Create a GpuState with no GPU context (CPU-only mode).
    pub fn cpu_only() -> Self {
        Self {
            context: RwLock::new(None),
        }
    }

    /// Get a reference to the current GPU context, if available.
    pub fn context(&self) -> Option<Arc<GpuContext>> {
        self.context.read().clone()
    }

    /// Switch to a different GPU adapter, or disable GPU (pass `None`).
    ///
    /// Returns the name of the newly selected adapter, or `None` if GPU
    /// was disabled or initialisation failed.
    pub fn set_adapter(&self, adapter_index: Option<usize>) -> Option<String> {
        match adapter_index {
            None => {
                info!("GPU acceleration disabled by user");
                *self.context.write() = None;
                None
            }
            Some(idx) => match GpuContext::new(Some(idx)) {
                Some(ctx) => {
                    let name = ctx.adapter_info.name.clone();
                    info!("Switched GPU adapter to: {name}");
                    *self.context.write() = Some(Arc::new(ctx));
                    Some(name)
                }
                None => {
                    warn!("Failed to initialise GPU adapter {idx} — falling back to CPU");
                    *self.context.write() = None;
                    None
                }
            },
        }
    }
}

impl Default for GpuState {
    fn default() -> Self {
        Self::new()
    }
}

/// Format identifier for colour conversion dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Nv12,
    Yuy2,
    Bgr24BottomUp,
}

/// Convert a frame using GPU if available, otherwise fall back to CPU.
///
/// This is the main entry point called from the capture callback.
pub fn convert_frame(
    gpu: Option<&Arc<GpuContext>>,
    format: PixelFormat,
    data: &[u8],
    width: usize,
    height: usize,
) -> Vec<u8> {
    // Try GPU path first
    if let Some(ctx) = gpu {
        let result = match format {
            PixelFormat::Nv12 => ctx.convert_nv12_to_rgb(data, width, height),
            PixelFormat::Yuy2 => ctx.convert_yuy2_to_rgb(data, width, height),
            PixelFormat::Bgr24BottomUp => ctx.convert_bgr_to_rgb(data, width, height),
        };
        if let Some(rgb) = result {
            return rgb;
        }
        // GPU conversion failed — fall through to CPU
        warn!(
            "GPU conversion failed for {:?}, falling back to CPU",
            format
        );
    }

    // CPU fallback
    match format {
        PixelFormat::Nv12 => super::graph::convert_nv12_to_rgb(data, width, height),
        PixelFormat::Yuy2 => super::graph::convert_yuy2_to_rgb(data, width, height),
        PixelFormat::Bgr24BottomUp => {
            super::graph::convert_bgr_bottom_up_to_rgb(data, width, height)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enumerate_adapters_returns_list() {
        // Should not panic even if no GPU is present
        let adapters = GpuContext::enumerate_adapters();
        // We cannot assert specific adapters exist in CI, but the function should work
        for adapter in &adapters {
            assert!(!adapter.name.is_empty());
            assert!(!adapter.backend.is_empty());
            assert!(!adapter.device_type.is_empty());
        }
    }

    #[test]
    fn gpu_state_cpu_only_returns_none_context() {
        let state = GpuState::cpu_only();
        assert!(state.context().is_none());
    }

    #[test]
    fn gpu_state_set_adapter_none_disables_gpu() {
        let state = GpuState::cpu_only();
        let result = state.set_adapter(None);
        assert!(result.is_none());
        assert!(state.context().is_none());
    }

    #[test]
    fn gpu_state_set_adapter_invalid_index_falls_back() {
        let state = GpuState::cpu_only();
        // Index 999 should not exist
        let result = state.set_adapter(Some(999));
        assert!(result.is_none());
        assert!(state.context().is_none());
    }

    #[test]
    fn convert_frame_uses_cpu_fallback_when_no_gpu() {
        // 2x2 BGR bottom-up: blue pixels in row 0, red pixels in row 1
        let bgr = vec![255u8, 0, 0, 255, 0, 0, 0, 0, 255, 0, 0, 255];

        let rgb = convert_frame(None, PixelFormat::Bgr24BottomUp, &bgr, 2, 2);

        // Row 0 of output = row 1 of input (flipped), BGR(0,0,255) -> RGB(255,0,0)
        assert_eq!(rgb[0], 255); // R
        assert_eq!(rgb[1], 0); // G
        assert_eq!(rgb[2], 0); // B
    }

    #[test]
    fn convert_frame_yuy2_cpu_fallback() {
        // Single macro-pixel: Y0=128, U=128, Y1=128, V=128 -> mid-grey
        let yuy2 = vec![128u8, 128, 128, 128];
        let rgb = convert_frame(None, PixelFormat::Yuy2, &yuy2, 2, 1);
        assert_eq!(rgb.len(), 6);
        // With Y=128, U=0 (128-128), V=0 (128-128) -> R=128, G=128, B=128
        assert_eq!(rgb[0], 128);
        assert_eq!(rgb[1], 128);
        assert_eq!(rgb[2], 128);
    }

    #[test]
    fn convert_frame_nv12_cpu_fallback() {
        // 2x2 NV12: all luma=128, U=128, V=128 -> mid-grey
        let mut nv12 = vec![128u8; 4]; // Y plane
        nv12.extend_from_slice(&[128, 128]); // UV plane
        let rgb = convert_frame(None, PixelFormat::Nv12, &nv12, 2, 2);
        assert_eq!(rgb.len(), 12);
        assert_eq!(rgb[0], 128);
        assert_eq!(rgb[1], 128);
        assert_eq!(rgb[2], 128);
    }

    #[test]
    fn pad_to_alignment_pads_correctly() {
        let data = vec![1u8, 2, 3];
        let padded = pad_to_alignment(&data, 4);
        assert_eq!(padded.len(), 4);
        assert_eq!(padded, vec![1, 2, 3, 0]);
    }

    #[test]
    fn pad_to_alignment_no_op_when_aligned() {
        let data = vec![1u8, 2, 3, 4];
        let padded = pad_to_alignment(&data, 4);
        assert_eq!(padded.len(), 4);
        assert_eq!(padded, data);
    }

    #[test]
    fn align_up_rounds_correctly() {
        assert_eq!(align_up(0, 4), 0);
        assert_eq!(align_up(1, 4), 4);
        assert_eq!(align_up(4, 4), 4);
        assert_eq!(align_up(5, 4), 8);
        assert_eq!(align_up(16, 16), 16);
        assert_eq!(align_up(17, 16), 32);
    }

    #[test]
    fn gpu_adapter_info_serialises_to_camel_case() {
        let info = GpuAdapterInfo {
            index: 0,
            name: "Test GPU".to_string(),
            backend: "Vulkan".to_string(),
            device_type: "DiscreteGpu".to_string(),
        };
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["deviceType"], "DiscreteGpu");
        assert!(json.get("device_type").is_none());
    }

    #[test]
    fn gpu_state_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<GpuState>();
    }

    #[test]
    fn pixel_format_debug_display() {
        assert_eq!(format!("{:?}", PixelFormat::Nv12), "Nv12");
        assert_eq!(format!("{:?}", PixelFormat::Yuy2), "Yuy2");
        assert_eq!(format!("{:?}", PixelFormat::Bgr24BottomUp), "Bgr24BottomUp");
    }
}
