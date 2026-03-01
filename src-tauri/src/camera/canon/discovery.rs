//! Canon camera enumeration via EDSDK.
//!
//! Discovers connected Canon cameras and maps them to `CameraDevice`
//! instances with `canon:<serial>` device IDs.

use crate::camera::error::Result;
use crate::camera::types::{CameraDevice, DeviceId};

use super::api::{CameraHandle, EdsSdkApi};

/// Discover Canon cameras using the provided SDK implementation.
///
/// Returns a list of `(CameraHandle, CameraDevice)` pairs so the caller
/// can maintain the handle-to-device mapping.
pub fn discover_cameras<S: EdsSdkApi>(sdk: &S) -> Result<Vec<(CameraHandle, CameraDevice)>> {
    let handles = sdk.camera_list()?;
    let mut devices = Vec::with_capacity(handles.len());

    for handle in handles {
        match sdk.get_device_info(handle) {
            Ok(info) => {
                let model = info.model_name();
                let device_id = make_device_id(&model, info.serial_number().as_deref());

                devices.push((
                    handle,
                    CameraDevice {
                        id: device_id,
                        name: model.clone(),
                        device_path: format!("edsdk://{model}"),
                        is_connected: true,
                    },
                ));
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to get device info for Canon camera {}: {e}",
                    handle.0
                );
            }
        }
    }

    Ok(devices)
}

/// Create a stable device ID for a Canon camera.
///
/// Uses `canon:<serial>` when a serial number is available, otherwise
/// falls back to `canon:<model_hash>`.
fn make_device_id(model: &str, serial: Option<&str>) -> DeviceId {
    match serial {
        Some(s) if !s.is_empty() => DeviceId::new(format!("canon:{s}")),
        _ => {
            let hash = simple_hash(model);
            DeviceId::new(format!("canon:{hash:016x}"))
        }
    }
}

/// Simple FNV-1a hash (same algorithm as in types.rs).
fn simple_hash(s: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in s.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::canon::mock::MockEdsSdk;

    #[test]
    fn discovers_zero_cameras() {
        let mock = MockEdsSdk::new();
        let result = discover_cameras(&mock).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn discovers_one_camera_with_serial() {
        let mock = MockEdsSdk::new().with_camera("Canon EOS R5", Some("ABC123"));
        let result = discover_cameras(&mock).unwrap();
        assert_eq!(result.len(), 1);

        let (handle, device) = &result[0];
        assert_eq!(handle.0, 0);
        assert_eq!(device.id, DeviceId::new("canon:ABC123"));
        assert_eq!(device.name, "Canon EOS R5");
        assert_eq!(device.device_path, "edsdk://Canon EOS R5");
        assert!(device.is_connected);
    }

    #[test]
    fn discovers_camera_without_serial_uses_hash() {
        let mock = MockEdsSdk::new().with_camera("Canon EOS R6", None);
        let result = discover_cameras(&mock).unwrap();
        assert_eq!(result.len(), 1);

        let (_, device) = &result[0];
        assert!(
            device.id.as_str().starts_with("canon:"),
            "ID should start with 'canon:': {}",
            device.id
        );
        // Hash-based fallback should be 16 hex chars
        let suffix = device.id.as_str().strip_prefix("canon:").unwrap();
        assert_eq!(suffix.len(), 16, "hash should be 16 hex chars: {suffix}");
    }

    #[test]
    fn discovers_multiple_cameras() {
        let mock = MockEdsSdk::new()
            .with_camera("Canon EOS R5", Some("SER001"))
            .with_camera("Canon EOS R6", Some("SER002"));
        let result = discover_cameras(&mock).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].1.id, DeviceId::new("canon:SER001"));
        assert_eq!(result[1].1.id, DeviceId::new("canon:SER002"));
    }

    #[test]
    fn same_camera_produces_same_id() {
        let mock = MockEdsSdk::new().with_camera("Canon EOS R5", Some("ABC123"));
        let r1 = discover_cameras(&mock).unwrap();
        let r2 = discover_cameras(&mock).unwrap();
        assert_eq!(r1[0].1.id, r2[0].1.id);
    }

    #[test]
    fn different_cameras_produce_different_ids() {
        let mock = MockEdsSdk::new()
            .with_camera("Canon EOS R5", Some("SER001"))
            .with_camera("Canon EOS R6", Some("SER002"));
        let result = discover_cameras(&mock).unwrap();
        assert_ne!(result[0].1.id, result[1].1.id);
    }

    #[test]
    fn device_id_format_is_canon_prefix() {
        let id = make_device_id("Canon EOS R5", Some("ABC123"));
        assert_eq!(id.as_str(), "canon:ABC123");
    }

    #[test]
    fn device_id_fallback_format() {
        let id = make_device_id("Canon EOS R5", None);
        assert!(id.as_str().starts_with("canon:"));
        assert!(id.as_str().len() > "canon:".len());
    }
}
