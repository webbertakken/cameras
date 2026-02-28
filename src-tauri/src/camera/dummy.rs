use std::collections::HashMap;
use std::sync::Mutex;

use crate::camera::backend::CameraBackend;
use crate::camera::error::{CameraError, Result};
use crate::camera::types::{
    CameraDevice, ControlDescriptor, ControlFlags, ControlId, ControlType, ControlValue, DeviceId,
    FormatDescriptor, HotplugEvent,
};

const DUMMY_DEVICE_ID: &str = "dummy:test:camera-001";
const DUMMY_DEVICE_NAME: &str = "Dummy Test Camera";

/// Control definition — static metadata for a simulated control.
struct ControlDef {
    id: ControlId,
    name: &'static str,
    min: i32,
    max: i32,
    default: i32,
    group: &'static str,
}

const CONTROL_DEFS: &[ControlDef] = &[
    ControlDef {
        id: ControlId::Brightness,
        name: "Brightness",
        min: 0,
        max: 255,
        default: 128,
        group: "image",
    },
    ControlDef {
        id: ControlId::Contrast,
        name: "Contrast",
        min: 0,
        max: 100,
        default: 50,
        group: "image",
    },
    ControlDef {
        id: ControlId::Saturation,
        name: "Saturation",
        min: 0,
        max: 200,
        default: 100,
        group: "image",
    },
    ControlDef {
        id: ControlId::Sharpness,
        name: "Sharpness",
        min: 0,
        max: 10,
        default: 5,
        group: "image",
    },
    ControlDef {
        id: ControlId::WhiteBalance,
        name: "White Balance",
        min: 2000,
        max: 9000,
        default: 6500,
        group: "exposure",
    },
];

/// Minimal valid JPEG — a 1x1 red pixel.
///
/// Generated from a standard JFIF structure.
fn test_pattern_jpeg() -> Vec<u8> {
    // Minimal 1x1 red pixel JPEG
    vec![
        0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01, 0x01, 0x00, 0x00,
        0x01, 0x00, 0x01, 0x00, 0x00, 0xFF, 0xDB, 0x00, 0x43, 0x00, 0x08, 0x06, 0x06, 0x07, 0x06,
        0x05, 0x08, 0x07, 0x07, 0x07, 0x09, 0x09, 0x08, 0x0A, 0x0C, 0x14, 0x0D, 0x0C, 0x0B, 0x0B,
        0x0C, 0x19, 0x12, 0x13, 0x0F, 0x14, 0x1D, 0x1A, 0x1F, 0x1E, 0x1D, 0x1A, 0x1C, 0x1C, 0x20,
        0x24, 0x2E, 0x27, 0x20, 0x22, 0x2C, 0x23, 0x1C, 0x1C, 0x28, 0x37, 0x29, 0x2C, 0x30, 0x31,
        0x34, 0x34, 0x34, 0x1F, 0x27, 0x39, 0x3D, 0x38, 0x32, 0x3C, 0x2E, 0x33, 0x34, 0x32, 0xFF,
        0xC0, 0x00, 0x0B, 0x08, 0x00, 0x01, 0x00, 0x01, 0x01, 0x01, 0x11, 0x00, 0xFF, 0xC4, 0x00,
        0x1F, 0x00, 0x00, 0x01, 0x05, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B,
        0xFF, 0xC4, 0x00, 0xB5, 0x10, 0x00, 0x02, 0x01, 0x03, 0x03, 0x02, 0x04, 0x03, 0x05, 0x05,
        0x04, 0x04, 0x00, 0x00, 0x01, 0x7D, 0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21,
        0x31, 0x41, 0x06, 0x13, 0x51, 0x61, 0x07, 0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xA1, 0x08,
        0x23, 0x42, 0xB1, 0xC1, 0x15, 0x52, 0xD1, 0xF0, 0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0A,
        0x16, 0x17, 0x18, 0x19, 0x1A, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2A, 0x34, 0x35, 0x36, 0x37,
        0x38, 0x39, 0x3A, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4A, 0x53, 0x54, 0x55, 0x56,
        0x57, 0x58, 0x59, 0x5A, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6A, 0x73, 0x74, 0x75,
        0x76, 0x77, 0x78, 0x79, 0x7A, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8A, 0x92, 0x93,
        0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9A, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7, 0xA8, 0xA9,
        0xAA, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7, 0xB8, 0xB9, 0xBA, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6,
        0xC7, 0xC8, 0xC9, 0xCA, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA, 0xE1, 0xE2,
        0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0xEA, 0xF1, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7,
        0xF8, 0xF9, 0xFA, 0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3F, 0x00, 0x7B, 0x94,
        0x11, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xD9,
    ]
}

/// A fake camera backend for testing without real hardware.
///
/// Provides simulated controls (Brightness, Contrast, Saturation, Sharpness,
/// White Balance) that store values in memory. Returns a minimal JPEG test
/// pattern for frame capture.
///
/// Enable via `DUMMY_CAMERA=1` environment variable.
pub struct DummyBackend {
    control_values: Mutex<HashMap<ControlId, i32>>,
}

impl DummyBackend {
    /// Create a new DummyBackend with all controls at their default values.
    pub fn new() -> Self {
        let mut values = HashMap::new();
        for def in CONTROL_DEFS {
            values.insert(def.id, def.default);
        }
        Self {
            control_values: Mutex::new(values),
        }
    }

    /// Whether the dummy camera is enabled via environment variable.
    pub fn is_enabled() -> bool {
        std::env::var("DUMMY_CAMERA").is_ok_and(|v| v == "1" || v == "true")
    }

    /// The stable device ID for the dummy camera.
    pub fn device_id() -> DeviceId {
        DeviceId::new(DUMMY_DEVICE_ID)
    }

    /// Return a minimal test pattern JPEG frame.
    pub fn test_frame() -> Vec<u8> {
        test_pattern_jpeg()
    }
}

impl CameraBackend for DummyBackend {
    fn enumerate_devices(&self) -> Result<Vec<CameraDevice>> {
        Ok(vec![CameraDevice {
            id: Self::device_id(),
            name: DUMMY_DEVICE_NAME.to_string(),
            device_path: "dummy://test-camera".to_string(),
            is_connected: true,
        }])
    }

    fn watch_hotplug(&self, _callback: Box<dyn Fn(HotplugEvent) + Send>) -> Result<()> {
        // Dummy backend does not generate hotplug events
        Ok(())
    }

    fn get_controls(&self, id: &DeviceId) -> Result<Vec<ControlDescriptor>> {
        if id != &Self::device_id() {
            return Err(CameraError::DeviceNotFound(id.to_string()));
        }

        let values = self.control_values.lock().unwrap();
        let descriptors = CONTROL_DEFS
            .iter()
            .map(|def| ControlDescriptor {
                id: def.id.as_id_str().to_string(),
                name: def.name.to_string(),
                control_type: ControlType::Slider,
                group: def.group.to_string(),
                min: Some(def.min),
                max: Some(def.max),
                step: Some(1),
                default: Some(def.default),
                current: values.get(&def.id).copied().unwrap_or(def.default),
                flags: ControlFlags {
                    supports_auto: false,
                    is_auto_enabled: false,
                    is_read_only: false,
                },
                supported: true,
            })
            .collect();

        Ok(descriptors)
    }

    fn get_control(&self, id: &DeviceId, control: &ControlId) -> Result<ControlValue> {
        if id != &Self::device_id() {
            return Err(CameraError::DeviceNotFound(id.to_string()));
        }

        let values = self.control_values.lock().unwrap();
        let val = values
            .get(control)
            .ok_or_else(|| CameraError::ControlQuery(format!("unknown control: {control:?}")))?;

        let def = CONTROL_DEFS
            .iter()
            .find(|d| d.id == *control)
            .ok_or_else(|| CameraError::ControlQuery(format!("no definition for {control:?}")))?;

        Ok(ControlValue::new(*val, Some(def.min), Some(def.max)))
    }

    fn set_control(&self, id: &DeviceId, control: &ControlId, value: ControlValue) -> Result<()> {
        if id != &Self::device_id() {
            return Err(CameraError::DeviceNotFound(id.to_string()));
        }

        let def = CONTROL_DEFS
            .iter()
            .find(|d| d.id == *control)
            .ok_or_else(|| {
                CameraError::ControlWrite(format!("unsupported control: {control:?}"))
            })?;

        let clamped = ControlValue::new(value.value(), Some(def.min), Some(def.max));
        self.control_values
            .lock()
            .unwrap()
            .insert(*control, clamped.value());

        Ok(())
    }

    fn get_formats(&self, id: &DeviceId) -> Result<Vec<FormatDescriptor>> {
        if id != &Self::device_id() {
            return Err(CameraError::DeviceNotFound(id.to_string()));
        }

        Ok(vec![FormatDescriptor {
            width: 1,
            height: 1,
            fps: 30.0,
            pixel_format: "JPEG".to_string(),
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dummy_backend_enumerates_one_device() {
        let backend = DummyBackend::new();
        let devices = backend.enumerate_devices().unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].name, "Dummy Test Camera");
        assert_eq!(devices[0].id, DummyBackend::device_id());
        assert!(devices[0].is_connected);
    }

    #[test]
    fn dummy_backend_device_id_is_stable() {
        let id1 = DummyBackend::device_id();
        let id2 = DummyBackend::device_id();
        assert_eq!(id1, id2);
        assert_eq!(id1.as_str(), "dummy:test:camera-001");
    }

    #[test]
    fn dummy_backend_has_five_controls() {
        let backend = DummyBackend::new();
        let controls = backend.get_controls(&DummyBackend::device_id()).unwrap();
        assert_eq!(controls.len(), 5);

        let ids: Vec<&str> = controls.iter().map(|c| c.id.as_str()).collect();
        assert!(ids.contains(&"brightness"));
        assert!(ids.contains(&"contrast"));
        assert!(ids.contains(&"saturation"));
        assert!(ids.contains(&"sharpness"));
        assert!(ids.contains(&"white_balance"));
    }

    #[test]
    fn dummy_backend_controls_have_correct_defaults() {
        let backend = DummyBackend::new();
        let controls = backend.get_controls(&DummyBackend::device_id()).unwrap();

        let brightness = controls.iter().find(|c| c.id == "brightness").unwrap();
        assert_eq!(brightness.default, Some(128));
        assert_eq!(brightness.current, 128);
        assert_eq!(brightness.min, Some(0));
        assert_eq!(brightness.max, Some(255));

        let wb = controls.iter().find(|c| c.id == "white_balance").unwrap();
        assert_eq!(wb.default, Some(6500));
        assert_eq!(wb.min, Some(2000));
        assert_eq!(wb.max, Some(9000));
    }

    #[test]
    fn dummy_backend_set_control_updates_value() {
        let backend = DummyBackend::new();
        let id = DummyBackend::device_id();

        backend
            .set_control(
                &id,
                &ControlId::Brightness,
                ControlValue::new(200, Some(0), Some(255)),
            )
            .unwrap();

        let val = backend.get_control(&id, &ControlId::Brightness).unwrap();
        assert_eq!(val.value(), 200);
    }

    #[test]
    fn dummy_backend_set_control_clamps_to_range() {
        let backend = DummyBackend::new();
        let id = DummyBackend::device_id();

        // Try to set brightness to 999 — should be clamped to 255
        backend
            .set_control(
                &id,
                &ControlId::Brightness,
                ControlValue::new(999, None, None),
            )
            .unwrap();

        let val = backend.get_control(&id, &ControlId::Brightness).unwrap();
        assert_eq!(val.value(), 255);
    }

    #[test]
    fn dummy_backend_get_controls_returns_error_for_unknown_device() {
        let backend = DummyBackend::new();
        let result = backend.get_controls(&DeviceId::new("nonexistent"));
        assert!(result.is_err());
    }

    #[test]
    fn dummy_backend_set_control_returns_error_for_unknown_device() {
        let backend = DummyBackend::new();
        let result = backend.set_control(
            &DeviceId::new("nonexistent"),
            &ControlId::Brightness,
            ControlValue::new(100, Some(0), Some(255)),
        );
        assert!(result.is_err());
    }

    #[test]
    fn dummy_backend_get_formats_returns_one_format() {
        let backend = DummyBackend::new();
        let formats = backend.get_formats(&DummyBackend::device_id()).unwrap();
        assert_eq!(formats.len(), 1);
        assert_eq!(formats[0].pixel_format, "JPEG");
    }

    #[test]
    fn dummy_backend_test_frame_is_valid_jpeg() {
        let frame = DummyBackend::test_frame();
        // JPEG files start with FF D8 and end with FF D9
        assert!(frame.len() > 4);
        assert_eq!(frame[0], 0xFF);
        assert_eq!(frame[1], 0xD8);
        assert_eq!(frame[frame.len() - 2], 0xFF);
        assert_eq!(frame[frame.len() - 1], 0xD9);
    }

    #[test]
    fn dummy_backend_get_control_returns_updated_value_in_descriptors() {
        let backend = DummyBackend::new();
        let id = DummyBackend::device_id();

        backend
            .set_control(
                &id,
                &ControlId::Contrast,
                ControlValue::new(75, Some(0), Some(100)),
            )
            .unwrap();

        let controls = backend.get_controls(&id).unwrap();
        let contrast = controls.iter().find(|c| c.id == "contrast").unwrap();
        assert_eq!(contrast.current, 75);
    }

    #[test]
    fn dummy_backend_watch_hotplug_succeeds() {
        let backend = DummyBackend::new();
        let result = backend.watch_hotplug(Box::new(|_| {}));
        assert!(result.is_ok());
    }

    #[test]
    fn dummy_backend_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<DummyBackend>();
    }
}
