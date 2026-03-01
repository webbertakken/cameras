//! EDSDK C type definitions, error codes, property IDs, and command constants.
//!
//! Values sourced from the Canon EDSDK C header files (EDSDK.h, EDSDKTypes.h).

/// EDSDK error code type.
pub type EdsError = u32;

/// Opaque handle to an EDSDK base reference.
pub type EdsBaseRef = *mut std::ffi::c_void;

/// Opaque handle to a camera reference.
pub type EdsCameraRef = *mut std::ffi::c_void;

/// Opaque handle to a camera list reference.
pub type EdsCameraListRef = *mut std::ffi::c_void;

/// Opaque handle to an EVF (electronic viewfinder) image reference.
pub type EdsEvfImageRef = *mut std::ffi::c_void;

/// Opaque handle to a stream reference (used for live view memory streams).
pub type EdsStreamRef = *mut std::ffi::c_void;

/// EDSDK property ID type.
pub type EdsPropertyID = u32;

/// EDSDK data type identifier.
pub type EdsDataType = u32;

/// EDSDK state event type.
pub type EdsStateEvent = u32;

/// EDSDK camera command type.
pub type EdsCameraCommand = u32;

/// Device information returned by `EdsGetDeviceInfo`.
#[derive(Debug, Clone)]
#[repr(C)]
pub struct EdsDeviceInfo {
    /// Model name (null-terminated).
    pub device_description: [u8; 256],
    /// Serial number (null-terminated, may be empty).
    pub body_id_ex: [u8; 256],
    /// Reserved fields.
    pub reserved1: u32,
    pub reserved2: u32,
}

impl EdsDeviceInfo {
    /// Extract the model name as a Rust string.
    pub fn model_name(&self) -> String {
        read_c_string(&self.device_description)
    }

    /// Extract the serial number as a Rust string. Returns `None` if empty.
    pub fn serial_number(&self) -> Option<String> {
        let s = read_c_string(&self.body_id_ex);
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    }
}

/// Property description — lists the available values for a property.
#[derive(Debug, Clone)]
pub struct EdsPropertyDesc {
    /// Number of valid values in `prop_desc`.
    pub num_elements: usize,
    /// Available values (EDSDK returns up to 128 entries).
    pub prop_desc: Vec<i32>,
}

// --- Error codes ---

/// Operation completed successfully.
pub const EDS_ERR_OK: EdsError = 0x00000000;
/// Unspecified internal SDK error.
pub const EDS_ERR_INTERNAL_ERROR: EdsError = 0x00000002;
/// Not enough memory to complete the operation.
pub const EDS_ERR_MEM_ALLOC_FAILED: EdsError = 0x00000003;
/// The device is busy; retry after a short delay.
pub const EDS_ERR_DEVICE_BUSY: EdsError = 0x00000081;
/// No camera session is open.
pub const EDS_ERR_SESSION_NOT_OPEN: EdsError = 0x00002003;
/// The requested object is not ready (live view startup).
pub const EDS_ERR_OBJECT_NOTREADY: EdsError = 0x0000A104;
/// The property is not available on this camera.
pub const EDS_ERR_PROPERTIES_UNAVAILABLE: EdsError = 0x00008D03;
/// A take-picture command failed.
pub const EDS_ERR_TAKE_PICTURE_AF_NG: EdsError = 0x00008D01;
/// The camera has been disconnected.
pub const EDS_ERR_COMM_DISCONNECTED: EdsError = 0x000000C1;
/// Invalid handle passed to the SDK.
pub const EDS_ERR_INVALID_HANDLE: EdsError = 0x00000061;

// --- Property IDs ---

/// ISO speed property.
pub const PROP_ID_ISO_SPEED: EdsPropertyID = 0x00000402;
/// Aperture (Av) property.
pub const PROP_ID_AV: EdsPropertyID = 0x00000405;
/// Shutter speed (Tv) property.
pub const PROP_ID_TV: EdsPropertyID = 0x00000404;
/// Exposure compensation property.
pub const PROP_ID_EXPOSURE_COMPENSATION: EdsPropertyID = 0x00000406;
/// White balance property.
pub const PROP_ID_WHITE_BALANCE: EdsPropertyID = 0x00000403;
/// Battery level property.
pub const PROP_ID_BATTERY_LEVEL: EdsPropertyID = 0x00000006;

// --- Camera commands ---

/// Start/stop EVF (electronic viewfinder) output.
pub const CAMERA_COMMAND_EVF_MODE: EdsCameraCommand = 0x00000002;
/// Take a picture.
pub const CAMERA_COMMAND_TAKE_PICTURE: EdsCameraCommand = 0x00000000;
/// Press the shutter button.
pub const CAMERA_COMMAND_PRESS_SHUTTER: EdsCameraCommand = 0x00000004;

// --- State events ---

/// Camera is shutting down (disconnect).
pub const STATE_EVENT_SHUTDOWN: EdsStateEvent = 0x00000001;

/// Read a null-terminated C string from a byte buffer.
fn read_c_string(buf: &[u8]) -> String {
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..end]).into_owned()
}

/// Map an EDSDK error code to a human-readable description.
pub fn error_description(code: EdsError) -> &'static str {
    match code {
        EDS_ERR_OK => "success",
        EDS_ERR_INTERNAL_ERROR => "EDSDK internal error",
        EDS_ERR_MEM_ALLOC_FAILED => "memory allocation failed",
        EDS_ERR_DEVICE_BUSY => "camera is busy — retry shortly",
        EDS_ERR_SESSION_NOT_OPEN => "no camera session is open",
        EDS_ERR_OBJECT_NOTREADY => "live view data not ready yet",
        EDS_ERR_PROPERTIES_UNAVAILABLE => "property not available on this camera",
        EDS_ERR_TAKE_PICTURE_AF_NG => "autofocus failed during capture",
        EDS_ERR_COMM_DISCONNECTED => "camera disconnected",
        EDS_ERR_INVALID_HANDLE => "invalid camera handle",
        _ => "unknown EDSDK error",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_have_correct_values() {
        assert_eq!(EDS_ERR_OK, 0);
        assert_eq!(EDS_ERR_DEVICE_BUSY, 0x00000081);
        assert_eq!(EDS_ERR_SESSION_NOT_OPEN, 0x00002003);
        assert_eq!(EDS_ERR_OBJECT_NOTREADY, 0x0000A104);
        assert_eq!(EDS_ERR_COMM_DISCONNECTED, 0x000000C1);
    }

    #[test]
    fn property_ids_have_correct_values() {
        assert_eq!(PROP_ID_ISO_SPEED, 0x00000402);
        assert_eq!(PROP_ID_AV, 0x00000405);
        assert_eq!(PROP_ID_TV, 0x00000404);
        assert_eq!(PROP_ID_EXPOSURE_COMPENSATION, 0x00000406);
        assert_eq!(PROP_ID_WHITE_BALANCE, 0x00000403);
    }

    #[test]
    fn command_constants_are_defined() {
        assert_eq!(CAMERA_COMMAND_EVF_MODE, 0x00000002);
        assert_eq!(CAMERA_COMMAND_TAKE_PICTURE, 0x00000000);
    }

    #[test]
    fn state_event_shutdown_is_defined() {
        assert_eq!(STATE_EVENT_SHUTDOWN, 0x00000001);
    }

    #[test]
    fn eds_device_info_model_name_reads_c_string() {
        let mut info = EdsDeviceInfo {
            device_description: [0u8; 256],
            body_id_ex: [0u8; 256],
            reserved1: 0,
            reserved2: 0,
        };
        let name = b"Canon EOS R5";
        info.device_description[..name.len()].copy_from_slice(name);

        assert_eq!(info.model_name(), "Canon EOS R5");
    }

    #[test]
    fn eds_device_info_serial_number_reads_c_string() {
        let mut info = EdsDeviceInfo {
            device_description: [0u8; 256],
            body_id_ex: [0u8; 256],
            reserved1: 0,
            reserved2: 0,
        };
        let serial = b"0123456789";
        info.body_id_ex[..serial.len()].copy_from_slice(serial);

        assert_eq!(info.serial_number(), Some("0123456789".to_string()));
    }

    #[test]
    fn eds_device_info_empty_serial_returns_none() {
        let info = EdsDeviceInfo {
            device_description: [0u8; 256],
            body_id_ex: [0u8; 256],
            reserved1: 0,
            reserved2: 0,
        };
        assert_eq!(info.serial_number(), None);
    }

    #[test]
    fn error_description_returns_human_readable_text() {
        assert_eq!(error_description(EDS_ERR_OK), "success");
        assert_eq!(
            error_description(EDS_ERR_DEVICE_BUSY),
            "camera is busy — retry shortly"
        );
        assert_eq!(
            error_description(EDS_ERR_COMM_DISCONNECTED),
            "camera disconnected"
        );
        assert_eq!(error_description(0xDEADBEEF), "unknown EDSDK error");
    }

    #[test]
    fn eds_property_desc_stores_values() {
        let desc = EdsPropertyDesc {
            num_elements: 3,
            prop_desc: vec![100, 200, 400],
        };
        assert_eq!(desc.num_elements, 3);
        assert_eq!(desc.prop_desc, vec![100, 200, 400]);
    }

    #[test]
    fn read_c_string_handles_empty_buffer() {
        let buf = [0u8; 10];
        assert_eq!(read_c_string(&buf), "");
    }

    #[test]
    fn read_c_string_handles_no_null_terminator() {
        let buf = [b'A', b'B', b'C'];
        assert_eq!(read_c_string(&buf), "ABC");
    }
}
