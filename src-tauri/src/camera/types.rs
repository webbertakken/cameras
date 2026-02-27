use serde::Serialize;
use std::fmt;

/// Stable camera identifier (VID:PID + serial or hash of device path).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct DeviceId(String);

impl DeviceId {
    /// Create a new `DeviceId` from a raw string.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Parse a Windows device path to extract VID:PID and produce a stable ID.
    ///
    /// Windows USB device paths typically contain `vid_XXXX&pid_XXXX`.
    /// Falls back to a hash of the full path if VID/PID cannot be extracted.
    pub fn from_device_path(path: &str) -> Self {
        let lower = path.to_lowercase();

        let vid = extract_field(&lower, "vid_");
        let pid = extract_field(&lower, "pid_");

        match (vid, pid) {
            (Some(v), Some(p)) => {
                // Try to find a serial number (segment after pid)
                if let Some(serial) = extract_serial(&lower) {
                    Self(format!("{v}:{p}:{serial}"))
                } else {
                    // Fallback: use a hash of the full path
                    let hash = simple_hash(path);
                    Self(format!("{v}:{p}:{hash:016x}"))
                }
            }
            _ => {
                // No VID/PID found — use full path hash
                let hash = simple_hash(path);
                Self(format!("unknown:{hash:016x}"))
            }
        }
    }

    /// Return the inner string representation.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Extract a 4-char hex field from a device path (e.g. "vid_" or "pid_").
fn extract_field(lower_path: &str, prefix: &str) -> Option<String> {
    let start = lower_path.find(prefix)? + prefix.len();
    let field: String = lower_path[start..].chars().take(4).collect();
    if field.len() == 4 && field.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(field)
    } else {
        None
    }
}

/// Extract a serial number from a Windows device path.
///
/// Device paths look like: `\\?\usb#vid_046d&pid_085e&mi_00#6&abc123#{guid}`
/// The serial is typically the segment after the second `#`.
fn extract_serial(lower_path: &str) -> Option<String> {
    let parts: Vec<&str> = lower_path.split('#').collect();
    if parts.len() >= 3 {
        let candidate = parts[2];
        // Serial should not be a GUID (starts with {) and not be empty
        if !candidate.is_empty() && !candidate.starts_with('{') && candidate.len() >= 4 {
            return Some(candidate.to_string());
        }
    }
    None
}

/// Simple FNV-1a hash for generating a stable fallback identifier.
fn simple_hash(s: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in s.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Discovered camera device.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraDevice {
    pub id: DeviceId,
    pub name: String,
    pub device_path: String,
    pub is_connected: bool,
}

/// Identifies a specific camera control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlId {
    // IAMCameraControl properties
    Pan,
    Tilt,
    Roll,
    Zoom,
    Exposure,
    Iris,
    Focus,
    // IAMVideoProcAmp properties
    Brightness,
    Contrast,
    Hue,
    Saturation,
    Sharpness,
    Gamma,
    ColorEnable,
    WhiteBalance,
    BacklightCompensation,
    Gain,
}

impl ControlId {
    /// Human-readable display name.
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Pan => "Pan",
            Self::Tilt => "Tilt",
            Self::Roll => "Roll",
            Self::Zoom => "Zoom",
            Self::Exposure => "Exposure",
            Self::Iris => "Iris",
            Self::Focus => "Focus",
            Self::Brightness => "Brightness",
            Self::Contrast => "Contrast",
            Self::Hue => "Hue",
            Self::Saturation => "Saturation",
            Self::Sharpness => "Sharpness",
            Self::Gamma => "Gamma",
            Self::ColorEnable => "Colour Enable",
            Self::WhiteBalance => "White Balance",
            Self::BacklightCompensation => "Backlight Compensation",
            Self::Gain => "Gain",
        }
    }

    /// Snake-case string identifier for IPC.
    pub fn as_id_str(self) -> &'static str {
        match self {
            Self::Pan => "pan",
            Self::Tilt => "tilt",
            Self::Roll => "roll",
            Self::Zoom => "zoom",
            Self::Exposure => "exposure",
            Self::Iris => "iris",
            Self::Focus => "focus",
            Self::Brightness => "brightness",
            Self::Contrast => "contrast",
            Self::Hue => "hue",
            Self::Saturation => "saturation",
            Self::Sharpness => "sharpness",
            Self::Gamma => "gamma",
            Self::ColorEnable => "color_enable",
            Self::WhiteBalance => "white_balance",
            Self::BacklightCompensation => "backlight_compensation",
            Self::Gain => "gain",
        }
    }

    /// Group for accordion UI section.
    pub fn group(self) -> &'static str {
        match self {
            Self::Brightness
            | Self::Contrast
            | Self::Saturation
            | Self::Hue
            | Self::Sharpness
            | Self::Gamma
            | Self::Gain => "image",
            Self::Exposure | Self::WhiteBalance | Self::BacklightCompensation => "exposure",
            Self::Focus | Self::Zoom | Self::Iris => "focus",
            Self::Pan | Self::Tilt | Self::Roll | Self::ColorEnable => "advanced",
        }
    }
}

impl ControlId {
    /// Parse a snake_case string into a ControlId.
    ///
    /// Returns `None` if the string does not match any known control.
    pub fn from_str_id(s: &str) -> Option<Self> {
        match s {
            "pan" => Some(Self::Pan),
            "tilt" => Some(Self::Tilt),
            "roll" => Some(Self::Roll),
            "zoom" => Some(Self::Zoom),
            "exposure" => Some(Self::Exposure),
            "iris" => Some(Self::Iris),
            "focus" => Some(Self::Focus),
            "brightness" => Some(Self::Brightness),
            "contrast" => Some(Self::Contrast),
            "hue" => Some(Self::Hue),
            "saturation" => Some(Self::Saturation),
            "sharpness" => Some(Self::Sharpness),
            "gamma" => Some(Self::Gamma),
            "color_enable" => Some(Self::ColorEnable),
            "white_balance" => Some(Self::WhiteBalance),
            "backlight_compensation" => Some(Self::BacklightCompensation),
            "gain" => Some(Self::Gain),
            _ => None,
        }
    }
}

/// Type of UI control widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlType {
    Slider,
    Toggle,
    Select,
}

/// Control capability flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlFlags {
    pub supports_auto: bool,
    pub is_auto_enabled: bool,
    pub is_read_only: bool,
}

/// Full metadata for a single camera control (matches frontend ControlDescriptor).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlDescriptor {
    pub id: String,
    pub name: String,
    pub control_type: ControlType,
    pub group: String,
    pub min: Option<i32>,
    pub max: Option<i32>,
    pub step: Option<i32>,
    pub default: Option<i32>,
    pub current: i32,
    pub flags: ControlFlags,
    pub supported: bool,
}

/// A control value, clamped to valid range on construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ControlValue(i32);

impl ControlValue {
    /// Create a new control value, clamping to [min, max] if bounds are provided.
    pub fn new(value: i32, min: Option<i32>, max: Option<i32>) -> Self {
        let mut v = value;
        if let Some(lo) = min {
            v = v.max(lo);
        }
        if let Some(hi) = max {
            v = v.min(hi);
        }
        Self(v)
    }

    /// Return the raw i32 value.
    pub fn value(self) -> i32 {
        self.0
    }
}

/// Camera video format descriptor.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FormatDescriptor {
    pub width: u32,
    pub height: u32,
    pub fps: f32,
    pub pixel_format: String,
}

impl Eq for FormatDescriptor {}

impl PartialOrd for FormatDescriptor {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for FormatDescriptor {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Sort by total pixels descending, then by fps descending
        let self_pixels = self.width * self.height;
        let other_pixels = other.width * other.height;
        other_pixels
            .cmp(&self_pixels)
            .then_with(|| {
                other
                    .fps
                    .partial_cmp(&self.fps)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| self.pixel_format.cmp(&other.pixel_format))
    }
}

/// Hot-plug event for device connection changes.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HotplugEvent {
    Connected(CameraDevice),
    Disconnected { id: DeviceId },
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- DeviceId tests ---

    #[test]
    fn device_id_creation_and_equality() {
        let id1 = DeviceId::new("046d:085e:abc123");
        let id2 = DeviceId::new("046d:085e:abc123");
        let id3 = DeviceId::new("046d:085e:different");

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn device_id_display() {
        let id = DeviceId::new("046d:085e:serial123");
        assert_eq!(id.to_string(), "046d:085e:serial123");
    }

    #[test]
    fn device_id_as_str() {
        let id = DeviceId::new("test-id");
        assert_eq!(id.as_str(), "test-id");
    }

    #[test]
    fn device_id_from_device_path_extracts_vid_pid() {
        let path = r"\\?\usb#vid_046d&pid_085e&mi_00#6&abc12345&0&0000#{guid}";
        let id = DeviceId::from_device_path(path);
        let s = id.as_str();
        assert!(s.starts_with("046d:085e:"), "got: {s}");
    }

    #[test]
    fn device_id_same_device_produces_same_id() {
        let path = r"\\?\usb#vid_046d&pid_085e&mi_00#serialnum#{guid}";
        let id1 = DeviceId::from_device_path(path);
        let id2 = DeviceId::from_device_path(path);
        assert_eq!(id1, id2);
    }

    #[test]
    fn device_id_different_vid_pid_produces_different_id() {
        let path1 = r"\\?\usb#vid_046d&pid_085e&mi_00#serial1#{guid}";
        let path2 = r"\\?\usb#vid_1234&pid_5678&mi_00#serial2#{guid}";
        let id1 = DeviceId::from_device_path(path1);
        let id2 = DeviceId::from_device_path(path2);
        assert_ne!(id1, id2);
    }

    #[test]
    fn device_id_fallback_when_no_vid_pid() {
        let path = r"\\?\some_weird_device_path";
        let id = DeviceId::from_device_path(path);
        assert!(id.as_str().starts_with("unknown:"), "got: {}", id.as_str());
    }

    #[test]
    fn device_id_fallback_when_no_serial() {
        // Path with VID/PID but no usable serial (short segment after #)
        let path = r"\\?\usb#vid_046d&pid_085e#ab#{guid}";
        let id = DeviceId::from_device_path(path);
        let s = id.as_str();
        // Should have vid:pid:hash format
        assert!(s.starts_with("046d:085e:"), "got: {s}");
    }

    // --- CameraDevice tests ---

    #[test]
    fn camera_device_construction() {
        let device = CameraDevice {
            id: DeviceId::new("046d:085e:serial"),
            name: "Logitech BRIO".to_string(),
            device_path: r"\\?\usb#vid_046d&pid_085e".to_string(),
            is_connected: true,
        };
        assert_eq!(device.name, "Logitech BRIO");
        assert!(device.is_connected);
        assert_eq!(device.id, DeviceId::new("046d:085e:serial"));
    }

    #[test]
    fn camera_device_serialises_to_json() {
        let device = CameraDevice {
            id: DeviceId::new("test"),
            name: "Test Cam".to_string(),
            device_path: "path".to_string(),
            is_connected: true,
        };
        let json = serde_json::to_value(&device).unwrap();
        assert_eq!(json["name"], "Test Cam");
        assert_eq!(json["isConnected"], true);
    }

    // --- ControlDescriptor tests ---

    #[test]
    fn control_descriptor_serialises_to_json_matching_frontend() {
        let desc = ControlDescriptor {
            id: "brightness".to_string(),
            name: "Brightness".to_string(),
            control_type: ControlType::Slider,
            group: "image".to_string(),
            min: Some(0),
            max: Some(255),
            step: Some(1),
            default: Some(128),
            current: 128,
            flags: ControlFlags {
                supports_auto: false,
                is_auto_enabled: false,
                is_read_only: false,
            },
            supported: true,
        };

        let json = serde_json::to_value(&desc).unwrap();
        assert_eq!(json["id"], "brightness");
        assert_eq!(json["controlType"], "slider");
        assert_eq!(json["group"], "image");
        assert_eq!(json["min"], 0);
        assert_eq!(json["max"], 255);
        assert_eq!(json["step"], 1);
        assert_eq!(json["default"], 128);
        assert_eq!(json["current"], 128);
        assert_eq!(json["flags"]["supportsAuto"], false);
        assert_eq!(json["flags"]["isAutoEnabled"], false);
        assert_eq!(json["flags"]["isReadOnly"], false);
        assert_eq!(json["supported"], true);
    }

    // --- ControlValue tests ---

    #[test]
    fn control_value_within_range() {
        let v = ControlValue::new(50, Some(0), Some(100));
        assert_eq!(v.value(), 50);
    }

    #[test]
    fn control_value_clamped_to_min() {
        let v = ControlValue::new(-10, Some(0), Some(100));
        assert_eq!(v.value(), 0);
    }

    #[test]
    fn control_value_clamped_to_max() {
        let v = ControlValue::new(200, Some(0), Some(100));
        assert_eq!(v.value(), 100);
    }

    #[test]
    fn control_value_no_bounds() {
        let v = ControlValue::new(42, None, None);
        assert_eq!(v.value(), 42);
    }

    // --- HotplugEvent tests ---

    #[test]
    fn hotplug_connected_variant() {
        let device = CameraDevice {
            id: DeviceId::new("test"),
            name: "Test".to_string(),
            device_path: "path".to_string(),
            is_connected: true,
        };
        let event = HotplugEvent::Connected(device);
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "connected");
        assert_eq!(json["name"], "Test");
    }

    #[test]
    fn hotplug_disconnected_variant() {
        let event = HotplugEvent::Disconnected {
            id: DeviceId::new("046d:085e:serial"),
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "disconnected");
        assert_eq!(json["id"], "046d:085e:serial");
    }

    // --- FormatDescriptor tests ---

    #[test]
    fn format_descriptor_equality() {
        let f1 = FormatDescriptor {
            width: 1920,
            height: 1080,
            fps: 30.0,
            pixel_format: "MJPG".to_string(),
        };
        let f2 = FormatDescriptor {
            width: 1920,
            height: 1080,
            fps: 30.0,
            pixel_format: "MJPG".to_string(),
        };
        assert_eq!(f1, f2);
    }

    #[test]
    fn format_descriptor_ordering_higher_res_first() {
        let hd = FormatDescriptor {
            width: 1920,
            height: 1080,
            fps: 30.0,
            pixel_format: "MJPG".to_string(),
        };
        let sd = FormatDescriptor {
            width: 640,
            height: 480,
            fps: 30.0,
            pixel_format: "MJPG".to_string(),
        };

        let mut formats = [sd.clone(), hd.clone()];
        formats.sort();

        assert_eq!(formats[0], hd);
        assert_eq!(formats[1], sd);
    }

    #[test]
    fn format_descriptor_ordering_higher_fps_first_at_same_res() {
        let f60 = FormatDescriptor {
            width: 1920,
            height: 1080,
            fps: 60.0,
            pixel_format: "MJPG".to_string(),
        };
        let f30 = FormatDescriptor {
            width: 1920,
            height: 1080,
            fps: 30.0,
            pixel_format: "MJPG".to_string(),
        };

        let mut formats = [f30.clone(), f60.clone()];
        formats.sort();

        assert_eq!(formats[0], f60);
        assert_eq!(formats[1], f30);
    }

    // --- ControlId tests ---

    #[test]
    fn control_id_groups_are_correct() {
        assert_eq!(ControlId::Brightness.group(), "image");
        assert_eq!(ControlId::Exposure.group(), "exposure");
        assert_eq!(ControlId::Focus.group(), "focus");
        assert_eq!(ControlId::Pan.group(), "advanced");
    }

    #[test]
    fn control_id_display_names() {
        assert_eq!(ControlId::WhiteBalance.display_name(), "White Balance");
        assert_eq!(
            ControlId::BacklightCompensation.display_name(),
            "Backlight Compensation"
        );
    }

    // --- ControlId::from_str_id tests ---

    #[test]
    fn from_str_id_parses_all_camera_controls() {
        assert_eq!(ControlId::from_str_id("pan"), Some(ControlId::Pan));
        assert_eq!(ControlId::from_str_id("tilt"), Some(ControlId::Tilt));
        assert_eq!(ControlId::from_str_id("roll"), Some(ControlId::Roll));
        assert_eq!(ControlId::from_str_id("zoom"), Some(ControlId::Zoom));
        assert_eq!(
            ControlId::from_str_id("exposure"),
            Some(ControlId::Exposure)
        );
        assert_eq!(ControlId::from_str_id("iris"), Some(ControlId::Iris));
        assert_eq!(ControlId::from_str_id("focus"), Some(ControlId::Focus));
    }

    #[test]
    fn from_str_id_parses_all_procamp_controls() {
        assert_eq!(
            ControlId::from_str_id("brightness"),
            Some(ControlId::Brightness)
        );
        assert_eq!(
            ControlId::from_str_id("contrast"),
            Some(ControlId::Contrast)
        );
        assert_eq!(ControlId::from_str_id("hue"), Some(ControlId::Hue));
        assert_eq!(
            ControlId::from_str_id("saturation"),
            Some(ControlId::Saturation)
        );
        assert_eq!(
            ControlId::from_str_id("sharpness"),
            Some(ControlId::Sharpness)
        );
        assert_eq!(ControlId::from_str_id("gamma"), Some(ControlId::Gamma));
        assert_eq!(
            ControlId::from_str_id("color_enable"),
            Some(ControlId::ColorEnable)
        );
        assert_eq!(
            ControlId::from_str_id("white_balance"),
            Some(ControlId::WhiteBalance)
        );
        assert_eq!(
            ControlId::from_str_id("backlight_compensation"),
            Some(ControlId::BacklightCompensation)
        );
        assert_eq!(ControlId::from_str_id("gain"), Some(ControlId::Gain));
    }

    #[test]
    fn from_str_id_returns_none_for_unknown() {
        assert_eq!(ControlId::from_str_id("nonexistent"), None);
        assert_eq!(ControlId::from_str_id(""), None);
        assert_eq!(ControlId::from_str_id("Brightness"), None);
    }

    #[test]
    fn from_str_id_roundtrips_with_as_id_str() {
        let all_controls = [
            ControlId::Pan,
            ControlId::Tilt,
            ControlId::Roll,
            ControlId::Zoom,
            ControlId::Exposure,
            ControlId::Iris,
            ControlId::Focus,
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
        for control in all_controls {
            let str_id = control.as_id_str();
            assert_eq!(
                ControlId::from_str_id(str_id),
                Some(control),
                "roundtrip failed for {str_id}"
            );
        }
    }
}
