use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tracing::{debug, error, info, warn};
use windows::core::{Interface, GUID};
use windows::Win32::Media::DirectShow::{IAMCameraControl, IAMVideoProcAmp};
use windows::Win32::Media::MediaFoundation::{
    CLSID_SystemDeviceEnum, CLSID_VideoInputDeviceCategory,
};
use windows::Win32::System::Com::StructuredStorage::IPropertyBag;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};
use windows::Win32::System::Variant::VARIANT;

use crate::camera::backend::CameraBackend;
use crate::camera::error::{CameraError, Result};
use crate::camera::types::{
    CameraDevice, ControlDescriptor, ControlFlags, ControlId, ControlType, ControlValue, DeviceId,
    FormatDescriptor, HotplugEvent,
};

/// Raw device info extracted from DirectShow enumeration.
#[derive(Debug, Clone)]
pub struct RawDeviceInfo {
    pub friendly_name: String,
    pub device_path: String,
}

/// Trait wrapping COM device enumeration for unit-testability.
pub trait DeviceEnumerator: Send + Sync {
    /// Enumerate raw device info from the system.
    fn enumerate_raw(&self) -> Result<Vec<RawDeviceInfo>>;
}

/// Real DirectShow device enumerator.
pub struct DirectShowEnumerator;

impl DirectShowEnumerator {
    pub fn new() -> Self {
        Self
    }
}

impl DeviceEnumerator for DirectShowEnumerator {
    fn enumerate_raw(&self) -> Result<Vec<RawDeviceInfo>> {
        unsafe { enumerate_directshow_devices() }
    }
}

/// Core DirectShow device enumeration.
///
/// # Safety
/// Calls COM APIs. Initialises COM (MTA) via a scoped guard.
unsafe fn enumerate_directshow_devices() -> Result<Vec<RawDeviceInfo>> {
    use windows::Win32::Media::DirectShow::ICreateDevEnum;
    use windows::Win32::System::Com::IMoniker;

    let _guard = ComGuard::init()?;

    let dev_enum: ICreateDevEnum =
        CoCreateInstance(&CLSID_SystemDeviceEnum, None, CLSCTX_INPROC_SERVER).map_err(|e| {
            CameraError::Enumeration(format!("CoCreateInstance(SystemDeviceEnum) failed: {e}"))
        })?;

    let mut enum_moniker = None;
    dev_enum
        .CreateClassEnumerator(&CLSID_VideoInputDeviceCategory, &mut enum_moniker, 0)
        .map_err(|e| CameraError::Enumeration(format!("CreateClassEnumerator failed: {e}")))?;

    let Some(enum_moniker) = enum_moniker else {
        return Ok(vec![]);
    };

    let mut devices = Vec::new();
    let mut moniker_array = [None; 1];

    loop {
        let hr = enum_moniker.Next(&mut moniker_array, None);

        if hr.is_err() {
            break;
        }

        let Some(moniker) = moniker_array[0].take() else {
            break;
        };

        let bag: IPropertyBag = match moniker.BindToStorage(
            None::<&windows::Win32::System::Com::IBindCtx>,
            None::<&IMoniker>,
        ) {
            Ok(b) => b,
            Err(e) => {
                warn!("BindToStorage failed for a device: {e}");
                continue;
            }
        };

        let friendly_name = read_property_string(&bag, "FriendlyName")
            .unwrap_or_else(|| "Unknown Camera".to_string());

        let device_path = read_property_string(&bag, "DevicePath").unwrap_or_default();

        debug!("Discovered device: name={friendly_name}, path={device_path}");

        devices.push(RawDeviceInfo {
            friendly_name,
            device_path,
        });
    }

    Ok(devices)
}

/// Read a string property from an `IPropertyBag`.
unsafe fn read_property_string(bag: &IPropertyBag, name: &str) -> Option<String> {
    use windows::core::BSTR;

    let prop_name = BSTR::from(name);
    let mut variant = VARIANT::default();

    bag.Read(
        windows::core::PCWSTR(prop_name.as_ptr()),
        &mut variant,
        None,
    )
    .ok()?;

    // Extract the BSTR value from the VARIANT union
    // VARIANT layout: Anonymous.Anonymous.Anonymous.bstrVal
    let bstr_ptr: *const *const u16 = std::ptr::addr_of!(variant)
        .cast::<u8>()
        .add(8) // offset to the union data in VARIANT
        .cast();
    let raw_bstr = *bstr_ptr;
    if raw_bstr.is_null() {
        return None;
    }

    // Read the BSTR length prefix (4 bytes before the pointer)
    let len_ptr = (raw_bstr as *const u8).sub(4) as *const u32;
    let byte_len = *len_ptr;
    let char_len = byte_len as usize / 2;

    let slice = std::slice::from_raw_parts(raw_bstr, char_len);
    Some(String::from_utf16_lossy(slice))
}

/// COM thread guard — ensures CoInitializeEx/CoUninitialize
/// pairing.
struct ComGuard;

impl ComGuard {
    fn init() -> Result<Self> {
        unsafe {
            let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
            if hr.is_err() {
                return Err(CameraError::ComInit(format!(
                    "CoInitializeEx failed: {hr:?}"
                )));
            }
        }
        Ok(Self)
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}

/// Windows camera backend using DirectShow.
pub struct WindowsBackend {
    enumerator: Box<dyn DeviceEnumerator>,
    /// Cache of known devices for diffing during hot-plug.
    known_devices: Arc<Mutex<HashMap<String, CameraDevice>>>,
}

impl WindowsBackend {
    /// Create a new backend with the real DirectShow
    /// enumerator.
    pub fn new() -> Self {
        Self {
            enumerator: Box::new(DirectShowEnumerator::new()),
            known_devices: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Create a backend with a custom enumerator (for
    /// testing).
    pub fn with_enumerator(enumerator: Box<dyn DeviceEnumerator>) -> Self {
        Self {
            enumerator,
            known_devices: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Convert raw device info into a `CameraDevice`.
    fn make_device(raw: &RawDeviceInfo) -> CameraDevice {
        let id = if raw.device_path.is_empty() {
            DeviceId::new(format!("name:{}", raw.friendly_name))
        } else {
            DeviceId::from_device_path(&raw.device_path)
        };

        CameraDevice {
            id,
            name: raw.friendly_name.clone(),
            device_path: raw.device_path.clone(),
            is_connected: true,
        }
    }
}

impl CameraBackend for WindowsBackend {
    fn enumerate_devices(&self) -> Result<Vec<CameraDevice>> {
        let raw_devices = self.enumerator.enumerate_raw()?;
        let devices: Vec<CameraDevice> = raw_devices.iter().map(Self::make_device).collect();

        let mut known = self.known_devices.lock().unwrap();
        known.clear();
        for dev in &devices {
            known.insert(dev.device_path.clone(), dev.clone());
        }

        info!("Enumerated {} camera device(s)", devices.len());
        Ok(devices)
    }

    fn watch_hotplug(&self, callback: Box<dyn Fn(HotplugEvent) + Send>) -> Result<()> {
        let enumerator_known = Arc::clone(&self.known_devices);

        std::thread::Builder::new()
            .name("camera-hotplug".to_string())
            .spawn(move || {
                if let Err(e) = run_hotplug_loop(enumerator_known, callback) {
                    error!("Hotplug loop exited with error: {e}");
                }
            })
            .map_err(|e| CameraError::Hotplug(format!("Failed to spawn hotplug thread: {e}")))?;

        Ok(())
    }

    fn get_controls(&self, id: &DeviceId) -> Result<Vec<ControlDescriptor>> {
        let known = self.known_devices.lock().unwrap();
        let device = known
            .values()
            .find(|d| &d.id == id)
            .ok_or_else(|| CameraError::DeviceNotFound(id.to_string()))?;
        let device_path = device.device_path.clone();
        drop(known);

        unsafe { query_device_controls(&device_path) }
    }

    fn get_control(&self, id: &DeviceId, control: &ControlId) -> Result<ControlValue> {
        let controls = self.get_controls(id)?;
        let desc = controls
            .iter()
            .find(|c| c.id == control.as_id_str())
            .ok_or_else(|| {
                CameraError::ControlQuery(format!("Control {control:?} not found on device {id}"))
            })?;

        Ok(ControlValue::new(desc.current, desc.min, desc.max))
    }

    fn set_control(&self, id: &DeviceId, control: &ControlId, value: ControlValue) -> Result<()> {
        let known = self.known_devices.lock().unwrap();
        let device = known
            .values()
            .find(|d| &d.id == id)
            .ok_or_else(|| CameraError::DeviceNotFound(id.to_string()))?;
        let device_path = device.device_path.clone();
        drop(known);

        unsafe { set_device_control(&device_path, control, value) }
    }

    fn get_formats(&self, id: &DeviceId) -> Result<Vec<FormatDescriptor>> {
        let known = self.known_devices.lock().unwrap();
        let device = known
            .values()
            .find(|d| &d.id == id)
            .ok_or_else(|| CameraError::DeviceNotFound(id.to_string()))?;
        let device_path = device.device_path.clone();
        drop(known);

        unsafe { query_device_formats(&device_path) }
    }
}

/// Helper: find a device filter by device path.
unsafe fn find_device_filter(
    device_path: &str,
) -> Result<windows::Win32::Media::DirectShow::IBaseFilter> {
    use windows::Win32::Media::DirectShow::ICreateDevEnum;
    use windows::Win32::System::Com::IMoniker;

    let _guard = ComGuard::init()?;

    let dev_enum: ICreateDevEnum =
        CoCreateInstance(&CLSID_SystemDeviceEnum, None, CLSCTX_INPROC_SERVER)
            .map_err(|e| CameraError::Enumeration(format!("CoCreateInstance failed: {e}")))?;

    let mut enum_moniker = None;
    dev_enum
        .CreateClassEnumerator(&CLSID_VideoInputDeviceCategory, &mut enum_moniker, 0)
        .map_err(|e| CameraError::Enumeration(format!("CreateClassEnumerator failed: {e}")))?;

    let Some(enum_moniker) = enum_moniker else {
        return Err(CameraError::DeviceNotFound(device_path.to_string()));
    };

    let mut moniker_array = [None; 1];

    loop {
        let hr = enum_moniker.Next(&mut moniker_array, None);
        if hr.is_err() {
            break;
        }

        let Some(moniker) = moniker_array[0].take() else {
            break;
        };

        let bag: IPropertyBag = match moniker.BindToStorage(
            None::<&windows::Win32::System::Com::IBindCtx>,
            None::<&IMoniker>,
        ) {
            Ok(b) => b,
            Err(_) => continue,
        };

        let path = read_property_string(&bag, "DevicePath").unwrap_or_default();
        if path != device_path {
            continue;
        }

        let filter: windows::Win32::Media::DirectShow::IBaseFilter = moniker
            .BindToObject(
                None::<&windows::Win32::System::Com::IBindCtx>,
                None::<&IMoniker>,
            )
            .map_err(|e| CameraError::Enumeration(format!("BindToObject failed: {e}")))?;

        return Ok(filter);
    }

    Err(CameraError::DeviceNotFound(device_path.to_string()))
}

/// All IAMCameraControl property variants (indices 0-6).
const CAMERA_CONTROL_IDS: [ControlId; 7] = [
    ControlId::Pan,
    ControlId::Tilt,
    ControlId::Roll,
    ControlId::Zoom,
    ControlId::Exposure,
    ControlId::Iris,
    ControlId::Focus,
];

/// All IAMVideoProcAmp property variants (indices 0-9).
const PROCAMP_CONTROL_IDS: [ControlId; 10] = [
    ControlId::Brightness,
    ControlId::Contrast,
    ControlId::Hue,
    ControlId::Saturation,
    ControlId::Sharpness,
    ControlId::Gamma,
    ControlId::ColorEnable,
    ControlId::WhiteBalance,
    ControlId::BacklightCompensation,
    ControlId::Gain,
];

/// Map a ControlId to its IAMCameraControl property index (0-6), or None if
/// the control belongs to IAMVideoProcAmp instead.
fn control_id_to_camera_property(id: &ControlId) -> Option<i32> {
    CAMERA_CONTROL_IDS
        .iter()
        .position(|c| c == id)
        .map(|i| i as i32)
}

/// Map a ControlId to its IAMVideoProcAmp property index (0-9), or None if
/// the control belongs to IAMCameraControl instead.
fn control_id_to_procamp_property(id: &ControlId) -> Option<i32> {
    PROCAMP_CONTROL_IDS
        .iter()
        .position(|c| c == id)
        .map(|i| i as i32)
}

/// Convert DirectShow capability and current flags into `ControlFlags`.
///
/// DirectShow uses bitmask flags where:
///  - bit 0 (0x1) = Auto supported / Auto enabled
///  - bit 1 (0x2) = Manual supported / Manual enabled
fn flags_to_control_flags(caps_flags: i32, cur_flags: i32) -> ControlFlags {
    ControlFlags {
        supports_auto: caps_flags & 0x1 != 0,
        is_auto_enabled: cur_flags & 0x1 != 0,
        is_read_only: false,
    }
}

/// Query controls from a device via DirectShow.
///
/// # Safety
/// Calls COM APIs.
unsafe fn query_device_controls(device_path: &str) -> Result<Vec<ControlDescriptor>> {
    let filter = find_device_filter(device_path)?;
    let mut controls = Vec::new();

    // Query IAMCameraControl (properties 0-6)
    if let Ok(cam_ctrl) = filter.cast::<IAMCameraControl>() {
        for (index, &control_id) in CAMERA_CONTROL_IDS.iter().enumerate() {
            let prop_index = index as i32;
            let mut min = 0i32;
            let mut max = 0i32;
            let mut step = 0i32;
            let mut default = 0i32;
            let mut caps_flags = 0i32;

            if cam_ctrl
                .GetRange(
                    prop_index,
                    &mut min,
                    &mut max,
                    &mut step,
                    &mut default,
                    &mut caps_flags,
                )
                .is_ok()
            {
                let mut current = 0i32;
                let mut cur_flags = 0i32;
                let _ = cam_ctrl.Get(prop_index, &mut current, &mut cur_flags);

                controls.push(make_control_descriptor(RawControlData {
                    control_id,
                    min,
                    max,
                    step,
                    default,
                    current,
                    caps_flags,
                    cur_flags,
                }));
            }
        }
    }

    // Query IAMVideoProcAmp (properties 0-9)
    if let Ok(video_proc) = filter.cast::<IAMVideoProcAmp>() {
        for (index, &control_id) in PROCAMP_CONTROL_IDS.iter().enumerate() {
            let prop_index = index as i32;
            let mut min = 0i32;
            let mut max = 0i32;
            let mut step = 0i32;
            let mut default = 0i32;
            let mut caps_flags = 0i32;

            if video_proc
                .GetRange(
                    prop_index,
                    &mut min,
                    &mut max,
                    &mut step,
                    &mut default,
                    &mut caps_flags,
                )
                .is_ok()
            {
                let mut current = 0i32;
                let mut cur_flags = 0i32;
                let _ = video_proc.Get(prop_index, &mut current, &mut cur_flags);

                controls.push(make_control_descriptor(RawControlData {
                    control_id,
                    min,
                    max,
                    step,
                    default,
                    current,
                    caps_flags,
                    cur_flags,
                }));
            }
        }
    }

    Ok(controls)
}

/// Raw control data from DirectShow for building a descriptor.
struct RawControlData {
    control_id: ControlId,
    min: i32,
    max: i32,
    step: i32,
    default: i32,
    current: i32,
    caps_flags: i32,
    cur_flags: i32,
}

/// Build a `ControlDescriptor` from raw DirectShow control
/// data.
fn make_control_descriptor(data: RawControlData) -> ControlDescriptor {
    let RawControlData {
        control_id,
        min,
        max,
        step,
        default,
        current,
        caps_flags,
        cur_flags,
    } = data;
    let control_type = if min == 0 && max == 1 {
        ControlType::Toggle
    } else {
        ControlType::Slider
    };

    ControlDescriptor {
        id: control_id.as_id_str().to_string(),
        name: control_id.display_name().to_string(),
        control_type,
        group: control_id.group().to_string(),
        min: Some(min),
        max: Some(max),
        step: Some(step),
        default: Some(default),
        current,
        flags: flags_to_control_flags(caps_flags, cur_flags),
        supported: true,
    }
}

/// Set a control value on a device via DirectShow.
///
/// # Safety
/// Calls COM APIs.
unsafe fn set_device_control(
    device_path: &str,
    control: &ControlId,
    value: ControlValue,
) -> Result<()> {
    let name = control.display_name();
    let filter = find_device_filter(device_path).map_err(|e| {
        CameraError::ControlWrite(format!("Failed to set {name}: device not found ({e})"))
    })?;

    if let Some(prop_index) = control_id_to_camera_property(control) {
        let cam_ctrl = filter.cast::<IAMCameraControl>().map_err(|e| {
            CameraError::ControlWrite(format!(
                "Failed to set {name}: IAMCameraControl not supported ({e})"
            ))
        })?;

        cam_ctrl
            .Set(prop_index, value.value(), 2) // 2 = manual mode
            .map_err(|e| {
                CameraError::ControlWrite(format!("Failed to set {name} to {}: {e}", value.value()))
            })?;
    } else if let Some(prop_index) = control_id_to_procamp_property(control) {
        let video_proc = filter.cast::<IAMVideoProcAmp>().map_err(|e| {
            CameraError::ControlWrite(format!(
                "Failed to set {name}: IAMVideoProcAmp not supported ({e})"
            ))
        })?;

        video_proc
            .Set(prop_index, value.value(), 2) // 2 = manual mode
            .map_err(|e| {
                CameraError::ControlWrite(format!("Failed to set {name} to {}: {e}", value.value()))
            })?;
    } else {
        return Err(CameraError::ControlWrite(format!(
            "Unknown control: {name}"
        )));
    }

    Ok(())
}

/// Query supported formats from a device.
///
/// # Safety
/// Calls COM APIs.
unsafe fn query_device_formats(device_path: &str) -> Result<Vec<FormatDescriptor>> {
    use windows::Win32::Media::DirectShow::IAMStreamConfig;
    use windows::Win32::Media::MediaFoundation::{FORMAT_VideoInfo, VIDEOINFOHEADER};

    let filter =
        find_device_filter(device_path).map_err(|e| CameraError::FormatQuery(e.to_string()))?;

    let pin_enum = filter
        .EnumPins()
        .map_err(|e| CameraError::FormatQuery(format!("EnumPins failed: {e}")))?;

    let mut formats = Vec::new();
    let mut pin_array = [None; 1];

    loop {
        let hr = pin_enum.Next(&mut pin_array, None);
        if hr.is_err() {
            break;
        }

        let Some(pin) = pin_array[0].take() else {
            break;
        };

        // Only consider output pins
        let dir = match pin.QueryDirection() {
            Ok(d) => d,
            Err(_) => continue,
        };
        // PINDIR_OUTPUT = 1
        if dir.0 != 1 {
            continue;
        }

        let Ok(stream_config) = pin.cast::<IAMStreamConfig>() else {
            continue;
        };

        let mut count = 0i32;
        let mut size = 0i32;
        if stream_config
            .GetNumberOfCapabilities(&mut count, &mut size)
            .is_err()
        {
            continue;
        }

        for i in 0..count {
            let mut scc = vec![0u8; size as usize];
            let mut mt_ptr = std::ptr::null_mut();
            if stream_config
                .GetStreamCaps(i, &mut mt_ptr, scc.as_mut_ptr())
                .is_err()
            {
                continue;
            }

            if mt_ptr.is_null() {
                continue;
            }

            let mt_ref = &*mt_ptr;
            if mt_ref.formattype == FORMAT_VideoInfo
                && !mt_ref.pbFormat.is_null()
                && mt_ref.cbFormat as usize >= size_of::<VIDEOINFOHEADER>()
            {
                let vih: &VIDEOINFOHEADER = &*(mt_ref.pbFormat as *const VIDEOINFOHEADER);

                let width = vih.bmiHeader.biWidth as u32;
                let height = vih.bmiHeader.biHeight.unsigned_abs();
                let fps = if vih.AvgTimePerFrame > 0 {
                    10_000_000.0 / vih.AvgTimePerFrame as f32
                } else {
                    0.0
                };

                let fourcc = fourcc_to_string(mt_ref.subtype);

                formats.push(FormatDescriptor {
                    width,
                    height,
                    fps,
                    pixel_format: fourcc,
                });
            }

            // Free the AM_MEDIA_TYPE
            if !mt_ref.pbFormat.is_null() {
                windows::Win32::System::Com::CoTaskMemFree(Some(mt_ref.pbFormat.cast()));
            }
            windows::Win32::System::Com::CoTaskMemFree(Some(
                (mt_ptr as *mut std::ffi::c_void).cast(),
            ));
        }
    }

    formats.sort();
    formats.dedup();

    Ok(formats)
}

/// Convert a media subtype GUID to a FourCC string.
fn fourcc_to_string(guid: GUID) -> String {
    let d1 = guid.data1;
    let bytes = d1.to_le_bytes();
    if bytes.iter().all(|b| b.is_ascii_graphic()) {
        String::from_utf8_lossy(&bytes).to_string()
    } else {
        format!("{d1:08X}")
    }
}

/// Context passed to the hotplug window procedure via GWLP_USERDATA.
struct HotplugContext {
    known_devices: Arc<Mutex<HashMap<String, CameraDevice>>>,
    callback: Box<dyn Fn(HotplugEvent) + Send>,
}

/// Diff known devices against a fresh enumeration, returning events for
/// any connected or disconnected devices. Updates `known` in place.
fn diff_devices(
    known: &mut HashMap<String, CameraDevice>,
    current: HashMap<String, CameraDevice>,
) -> Vec<HotplugEvent> {
    let mut events = Vec::new();

    for (path, device) in &current {
        if !known.contains_key(path) {
            events.push(HotplugEvent::Connected(device.clone()));
        }
    }
    for (path, device) in known.iter() {
        if !current.contains_key(path) {
            events.push(HotplugEvent::Disconnected {
                id: device.id.clone(),
            });
        }
    }

    *known = current;
    events
}

/// Re-enumerate devices and fire hotplug events for any changes.
fn handle_device_change(ctx: &HotplugContext) {
    let current_raw = match unsafe { enumerate_directshow_devices() } {
        Ok(devs) => devs,
        Err(e) => {
            error!("Failed to re-enumerate devices during hotplug: {e}");
            return;
        }
    };

    let current: HashMap<String, CameraDevice> = current_raw
        .iter()
        .map(|raw| {
            let dev = WindowsBackend::make_device(raw);
            (dev.device_path.clone(), dev)
        })
        .collect();

    let mut known = ctx.known_devices.lock().unwrap();
    let events = diff_devices(&mut known, current);
    drop(known);

    for event in events {
        info!("Hotplug event: {event:?}");
        (ctx.callback)(event);
    }
}

/// Run the hot-plug detection message loop.
fn run_hotplug_loop(
    known_devices: Arc<Mutex<HashMap<String, CameraDevice>>>,
    callback: Box<dyn Fn(HotplugEvent) + Send>,
) -> Result<()> {
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DispatchMessageW, GetMessageW, RegisterClassW,
        RegisterDeviceNotificationW, SetWindowLongPtrW, TranslateMessage, CS_HREDRAW, CS_VREDRAW,
        DEV_BROADCAST_DEVICEINTERFACE_W, GWLP_USERDATA, HWND_MESSAGE, MSG,
        REGISTER_NOTIFICATION_FLAGS, WINDOW_EX_STYLE, WINDOW_STYLE, WNDCLASSW,
    };

    unsafe {
        let _guard = ComGuard::init()?;

        let class_name = windows::core::w!("CameraHotplugWnd");
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(hotplug_wnd_proc),
            lpszClassName: class_name,
            ..Default::default()
        };

        RegisterClassW(&wc);

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class_name,
            windows::core::w!("CameraHotplug"),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            None,
            None,
        )
        .map_err(|e| CameraError::Hotplug(format!("CreateWindowExW failed: {e}")))?;

        // Store context on the HWND so wnd_proc can access it
        let ctx = Box::new(HotplugContext {
            known_devices,
            callback,
        });
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(ctx) as isize);

        // KSCATEGORY_VIDEO_CAMERA GUID
        let guid = GUID::from_values(
            0xe5323777,
            0xf976,
            0x4f5b,
            [0x9b, 0x55, 0xb9, 0x46, 0x99, 0xc4, 0x6e, 0x44],
        );

        let filter = DEV_BROADCAST_DEVICEINTERFACE_W {
            dbcc_size: std::mem::size_of::<DEV_BROADCAST_DEVICEINTERFACE_W>() as u32,
            dbcc_devicetype: 5u32, // DBT_DEVTYP_DEVICEINTERFACE
            dbcc_classguid: guid,
            ..Default::default()
        };

        let _notification = RegisterDeviceNotificationW(
            windows::Win32::Foundation::HANDLE(hwnd.0 as _),
            std::ptr::addr_of!(filter).cast(),
            REGISTER_NOTIFICATION_FLAGS(0x00000004),
        )
        .map_err(|e| CameraError::Hotplug(format!("RegisterDeviceNotificationW failed: {e}")))?;

        info!("Hot-plug detection started");

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, Some(hwnd), 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        Ok(())
    }
}

/// Window procedure for the hot-plug detection window.
unsafe extern "system" fn hotplug_wnd_proc(
    hwnd: windows::Win32::Foundation::HWND,
    msg: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::UI::WindowsAndMessaging::{
        DefWindowProcW, GetWindowLongPtrW, GWLP_USERDATA, WM_DEVICECHANGE,
    };

    // DBT_DEVICEARRIVAL = 0x8000, DBT_DEVICEREMOVECOMPLETE = 0x8004
    if msg == WM_DEVICECHANGE && (wparam.0 == 0x8000 || wparam.0 == 0x8004) {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
        if ptr != 0 {
            let ctx = &*(ptr as *const HotplugContext);
            handle_device_change(ctx);
        }
    }

    DefWindowProcW(hwnd, msg, wparam, lparam)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockEnumerator {
        devices: Vec<RawDeviceInfo>,
    }

    impl DeviceEnumerator for MockEnumerator {
        fn enumerate_raw(&self) -> Result<Vec<RawDeviceInfo>> {
            Ok(self.devices.clone())
        }
    }

    #[test]
    fn enumerate_devices_returns_camera_devices() {
        let backend = WindowsBackend::with_enumerator(Box::new(MockEnumerator {
            devices: vec![RawDeviceInfo {
                friendly_name: "Test Camera".to_string(),
                device_path: r"\\?\usb#vid_046d&pid_085e&mi_00#serial123#{guid}".to_string(),
            }],
        }));

        let devices = backend.enumerate_devices().unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].name, "Test Camera");
        assert!(devices[0].is_connected);
    }

    #[test]
    fn enumerate_devices_with_no_cameras() {
        let backend = WindowsBackend::with_enumerator(Box::new(MockEnumerator { devices: vec![] }));

        let devices = backend.enumerate_devices().unwrap();
        assert!(devices.is_empty());
    }

    #[test]
    fn enumerate_devices_have_non_empty_names() {
        let backend = WindowsBackend::with_enumerator(Box::new(MockEnumerator {
            devices: vec![
                RawDeviceInfo {
                    friendly_name: "Logitech BRIO".to_string(),
                    device_path: "path1".to_string(),
                },
                RawDeviceInfo {
                    friendly_name: "OBS Virtual Camera".to_string(),
                    device_path: "path2".to_string(),
                },
            ],
        }));

        let devices = backend.enumerate_devices().unwrap();
        for device in &devices {
            assert!(!device.name.is_empty(), "Device name should not be empty");
        }
    }

    #[test]
    fn enumerate_devices_have_valid_device_ids() {
        let backend = WindowsBackend::with_enumerator(Box::new(MockEnumerator {
            devices: vec![RawDeviceInfo {
                friendly_name: "Camera".to_string(),
                device_path: r"\\?\usb#vid_1234&pid_5678#serial#{guid}".to_string(),
            }],
        }));

        let devices = backend.enumerate_devices().unwrap();
        assert!(!devices[0].id.as_str().is_empty());
        assert!(
            devices[0].id.as_str().contains("1234"),
            "DeviceId should contain VID"
        );
    }

    #[test]
    fn get_controls_errors_for_unknown_device() {
        let backend = WindowsBackend::with_enumerator(Box::new(MockEnumerator { devices: vec![] }));

        let result = backend.get_controls(&DeviceId::new("nonexistent"));
        assert!(result.is_err());
    }

    #[test]
    fn get_formats_errors_for_unknown_device() {
        let backend = WindowsBackend::with_enumerator(Box::new(MockEnumerator { devices: vec![] }));

        let result = backend.get_formats(&DeviceId::new("nonexistent"));
        assert!(result.is_err());
    }

    #[test]
    fn make_device_from_raw_info() {
        let raw = RawDeviceInfo {
            friendly_name: "My Camera".to_string(),
            device_path: r"\\?\usb#vid_046d&pid_085e#serial#{guid}".to_string(),
        };

        let device = WindowsBackend::make_device(&raw);
        assert_eq!(device.name, "My Camera");
        assert!(device.is_connected);
        assert!(device.id.as_str().starts_with("046d:085e:"));
    }

    #[test]
    fn make_device_fallback_for_empty_path() {
        let raw = RawDeviceInfo {
            friendly_name: "Virtual Cam".to_string(),
            device_path: String::new(),
        };

        let device = WindowsBackend::make_device(&raw);
        assert_eq!(device.name, "Virtual Cam");
        assert!(
            device.id.as_str().starts_with("name:"),
            "Empty path should produce name-based ID"
        );
    }

    #[test]
    fn make_control_descriptor_builds_slider() {
        let desc = make_control_descriptor(RawControlData {
            control_id: ControlId::Brightness,
            min: 0,
            max: 255,
            step: 1,
            default: 128,
            current: 100,
            caps_flags: 0x03,
            cur_flags: 0x00,
        });

        assert_eq!(desc.id, "brightness");
        assert_eq!(desc.name, "Brightness");
        assert_eq!(desc.control_type, ControlType::Slider);
        assert_eq!(desc.group, "image");
        assert_eq!(desc.min, Some(0));
        assert_eq!(desc.max, Some(255));
        assert_eq!(desc.step, Some(1));
        assert_eq!(desc.default, Some(128));
        assert_eq!(desc.current, 100);
        assert!(desc.flags.supports_auto);
        assert!(!desc.flags.is_auto_enabled);
        assert!(desc.supported);
    }

    #[test]
    fn make_control_descriptor_builds_toggle() {
        let desc = make_control_descriptor(RawControlData {
            control_id: ControlId::ColorEnable,
            min: 0,
            max: 1,
            step: 1,
            default: 1,
            current: 1,
            caps_flags: 0x00,
            cur_flags: 0x00,
        });

        assert_eq!(desc.control_type, ControlType::Toggle);
    }

    #[test]
    fn make_control_descriptor_auto_enabled() {
        let desc = make_control_descriptor(RawControlData {
            control_id: ControlId::Exposure,
            min: -11,
            max: 1,
            step: 1,
            default: -5,
            current: -5,
            caps_flags: 0x03,
            cur_flags: 0x01,
        });

        assert!(desc.flags.supports_auto);
        assert!(desc.flags.is_auto_enabled);
    }

    #[test]
    fn watch_hotplug_accepts_send_callback() {
        let _callback: Box<dyn Fn(HotplugEvent) + Send> = Box::new(|_| {});
        assert!(true);
    }

    // --- control_id_to_camera_property tests ---

    #[test]
    fn camera_property_maps_pan_to_0() {
        assert_eq!(control_id_to_camera_property(&ControlId::Pan), Some(0));
    }

    #[test]
    fn camera_property_maps_tilt_to_1() {
        assert_eq!(control_id_to_camera_property(&ControlId::Tilt), Some(1));
    }

    #[test]
    fn camera_property_maps_roll_to_2() {
        assert_eq!(control_id_to_camera_property(&ControlId::Roll), Some(2));
    }

    #[test]
    fn camera_property_maps_zoom_to_3() {
        assert_eq!(control_id_to_camera_property(&ControlId::Zoom), Some(3));
    }

    #[test]
    fn camera_property_maps_exposure_to_4() {
        assert_eq!(control_id_to_camera_property(&ControlId::Exposure), Some(4));
    }

    #[test]
    fn camera_property_maps_iris_to_5() {
        assert_eq!(control_id_to_camera_property(&ControlId::Iris), Some(5));
    }

    #[test]
    fn camera_property_maps_focus_to_6() {
        assert_eq!(control_id_to_camera_property(&ControlId::Focus), Some(6));
    }

    #[test]
    fn camera_property_returns_none_for_procamp_controls() {
        assert_eq!(control_id_to_camera_property(&ControlId::Brightness), None);
        assert_eq!(control_id_to_camera_property(&ControlId::Contrast), None);
        assert_eq!(control_id_to_camera_property(&ControlId::Gain), None);
        assert_eq!(
            control_id_to_camera_property(&ControlId::WhiteBalance),
            None
        );
    }

    // --- control_id_to_procamp_property tests ---

    #[test]
    fn procamp_property_maps_brightness_to_0() {
        assert_eq!(
            control_id_to_procamp_property(&ControlId::Brightness),
            Some(0)
        );
    }

    #[test]
    fn procamp_property_maps_contrast_to_1() {
        assert_eq!(
            control_id_to_procamp_property(&ControlId::Contrast),
            Some(1)
        );
    }

    #[test]
    fn procamp_property_maps_hue_to_2() {
        assert_eq!(control_id_to_procamp_property(&ControlId::Hue), Some(2));
    }

    #[test]
    fn procamp_property_maps_saturation_to_3() {
        assert_eq!(
            control_id_to_procamp_property(&ControlId::Saturation),
            Some(3)
        );
    }

    #[test]
    fn procamp_property_maps_sharpness_to_4() {
        assert_eq!(
            control_id_to_procamp_property(&ControlId::Sharpness),
            Some(4)
        );
    }

    #[test]
    fn procamp_property_maps_gamma_to_5() {
        assert_eq!(control_id_to_procamp_property(&ControlId::Gamma), Some(5));
    }

    #[test]
    fn procamp_property_maps_color_enable_to_6() {
        assert_eq!(
            control_id_to_procamp_property(&ControlId::ColorEnable),
            Some(6)
        );
    }

    #[test]
    fn procamp_property_maps_white_balance_to_7() {
        assert_eq!(
            control_id_to_procamp_property(&ControlId::WhiteBalance),
            Some(7)
        );
    }

    #[test]
    fn procamp_property_maps_backlight_compensation_to_8() {
        assert_eq!(
            control_id_to_procamp_property(&ControlId::BacklightCompensation),
            Some(8)
        );
    }

    #[test]
    fn procamp_property_maps_gain_to_9() {
        assert_eq!(control_id_to_procamp_property(&ControlId::Gain), Some(9));
    }

    #[test]
    fn procamp_property_returns_none_for_camera_controls() {
        assert_eq!(control_id_to_procamp_property(&ControlId::Pan), None);
        assert_eq!(control_id_to_procamp_property(&ControlId::Tilt), None);
        assert_eq!(control_id_to_procamp_property(&ControlId::Focus), None);
        assert_eq!(control_id_to_procamp_property(&ControlId::Exposure), None);
    }

    // --- flags_to_control_flags tests ---

    #[test]
    fn flags_auto_supported_and_enabled() {
        let flags = flags_to_control_flags(0x01, 0x01);
        assert!(flags.supports_auto);
        assert!(flags.is_auto_enabled);
        assert!(!flags.is_read_only);
    }

    #[test]
    fn flags_auto_supported_but_manual_active() {
        let flags = flags_to_control_flags(0x03, 0x02);
        assert!(flags.supports_auto);
        assert!(!flags.is_auto_enabled);
    }

    #[test]
    fn flags_manual_only() {
        let flags = flags_to_control_flags(0x02, 0x02);
        assert!(!flags.supports_auto);
        assert!(!flags.is_auto_enabled);
    }

    #[test]
    fn flags_zero_means_no_auto() {
        let flags = flags_to_control_flags(0x00, 0x00);
        assert!(!flags.supports_auto);
        assert!(!flags.is_auto_enabled);
        assert!(!flags.is_read_only);
    }

    #[test]
    fn fourcc_to_string_converts_known_formats() {
        let mjpg_guid = GUID::from_values(
            0x47504A4D,
            0x0000,
            0x0010,
            [0x80, 0x00, 0x00, 0xaa, 0x00, 0x38, 0x9b, 0x71],
        );
        assert_eq!(fourcc_to_string(mjpg_guid), "MJPG");

        let yuy2_guid = GUID::from_values(
            0x32595559,
            0x0000,
            0x0010,
            [0x80, 0x00, 0x00, 0xaa, 0x00, 0x38, 0x9b, 0x71],
        );
        assert_eq!(fourcc_to_string(yuy2_guid), "YUY2");
    }

    // --- diff_devices tests ---

    fn make_test_device(path: &str, name: &str) -> CameraDevice {
        CameraDevice {
            id: DeviceId::new(name),
            name: name.to_string(),
            device_path: path.to_string(),
            is_connected: true,
        }
    }

    #[test]
    fn diff_devices_detects_new_device() {
        let mut known = HashMap::new();
        let mut current = HashMap::new();
        let dev = make_test_device("path_a", "Camera A");
        current.insert("path_a".to_string(), dev);

        let events = diff_devices(&mut known, current);

        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], HotplugEvent::Connected(d) if d.name == "Camera A"));
        assert_eq!(known.len(), 1);
    }

    #[test]
    fn diff_devices_detects_removed_device() {
        let mut known = HashMap::new();
        let dev = make_test_device("path_b", "Camera B");
        known.insert("path_b".to_string(), dev);
        let current = HashMap::new();

        let events = diff_devices(&mut known, current);

        assert_eq!(events.len(), 1);
        assert!(
            matches!(&events[0], HotplugEvent::Disconnected { id } if id.as_str() == "Camera B")
        );
        assert!(known.is_empty());
    }

    #[test]
    fn diff_devices_no_changes() {
        let dev = make_test_device("path_c", "Camera C");
        let mut known = HashMap::from([("path_c".to_string(), dev.clone())]);
        let current = HashMap::from([("path_c".to_string(), dev)]);

        let events = diff_devices(&mut known, current);

        assert!(events.is_empty());
        assert_eq!(known.len(), 1);
    }

    #[test]
    fn diff_devices_simultaneous_add_and_remove() {
        let old_dev = make_test_device("path_old", "Old Camera");
        let new_dev = make_test_device("path_new", "New Camera");
        let mut known = HashMap::from([("path_old".to_string(), old_dev)]);
        let current = HashMap::from([("path_new".to_string(), new_dev)]);

        let events = diff_devices(&mut known, current);

        assert_eq!(events.len(), 2);
        let has_connect = events
            .iter()
            .any(|e| matches!(e, HotplugEvent::Connected(d) if d.name == "New Camera"));
        let has_disconnect = events
            .iter()
            .any(|e| matches!(e, HotplugEvent::Disconnected { id } if id.as_str() == "Old Camera"));
        assert!(has_connect, "Expected Connected event for New Camera");
        assert!(has_disconnect, "Expected Disconnected event for Old Camera");
        assert_eq!(known.len(), 1);
        assert!(known.contains_key("path_new"));
    }

    #[test]
    fn diff_devices_both_empty() {
        let mut known = HashMap::new();
        let current = HashMap::new();

        let events = diff_devices(&mut known, current);

        assert!(events.is_empty());
    }
}
