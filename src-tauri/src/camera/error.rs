use thiserror::Error;

/// Camera subsystem errors.
#[derive(Debug, Clone, Error)]
pub enum CameraError {
    #[error("device not found: {0}")]
    DeviceNotFound(String),

    #[error("COM initialisation failed: {0}")]
    ComInit(String),

    #[error("device enumeration failed: {0}")]
    Enumeration(String),

    #[error("control query failed: {0}")]
    ControlQuery(String),

    #[error("control write failed: {0}")]
    ControlWrite(String),

    #[error("format query failed: {0}")]
    FormatQuery(String),

    #[error("hotplug registration failed: {0}")]
    Hotplug(String),

    #[error("Canon SDK error: {0}")]
    CanonSdkError(String),

    #[error("Canon session not open: {0}")]
    CanonSessionNotOpen(String),

    #[error("Canon device busy: {0}")]
    CanonDeviceBusy(String),
}

/// Convenience Result alias.
pub type Result<T> = std::result::Result<T, CameraError>;

/// Known Windows HRESULT codes and their user-friendly translations.
const HRESULT_TRANSLATIONS: &[(&str, &str)] = &[
    ("0x800705AA", "Camera is in use by another application"),
    (
        "0x80070005",
        "Access denied — close other camera apps and retry",
    ),
    ("0x80004005", "Camera returned an unspecified error"),
    ("0x80070020", "Camera is locked by another process"),
    (
        "0x8007001F",
        "A device attached to the system is not functioning",
    ),
];

/// Replace known HRESULT codes and Canon SDK messages with human-friendly text.
pub fn humanise_error(msg: &str) -> String {
    for &(code, friendly) in HRESULT_TRANSLATIONS {
        if msg.contains(code) {
            return friendly.to_string();
        }
    }
    // Canon SDK error translations
    if msg.contains("camera is busy") {
        return "Canon camera is busy — wait a moment and try again".to_string();
    }
    if msg.contains("session not open") || msg.contains("SESSION_NOT_OPEN") {
        return "Canon camera session is not open — reconnect the camera".to_string();
    }
    if msg.contains("camera disconnected") || msg.contains("COMM_DISCONNECTED") {
        return "Canon camera was disconnected".to_string();
    }
    msg.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn humanise_translates_insufficient_resources() {
        let msg = "CoCreateInstance failed: 0x800705AA";
        assert_eq!(
            humanise_error(msg),
            "Camera is in use by another application"
        );
    }

    #[test]
    fn humanise_translates_access_denied() {
        let msg = "BindToObject failed: 0x80070005";
        assert_eq!(
            humanise_error(msg),
            "Access denied — close other camera apps and retry"
        );
    }

    #[test]
    fn humanise_translates_sharing_violation() {
        let msg = "something 0x80070020 happened";
        assert_eq!(humanise_error(msg), "Camera is locked by another process");
    }

    #[test]
    fn humanise_passes_through_unknown_errors() {
        let msg = "some random error without HRESULT";
        assert_eq!(humanise_error(msg), msg);
    }

    #[test]
    fn humanise_translates_canon_busy() {
        let msg = "Canon SDK error: camera is busy — retry shortly";
        assert!(humanise_error(msg).contains("Canon camera is busy"));
    }

    #[test]
    fn humanise_translates_canon_session_not_open() {
        let msg = "no camera session not open";
        assert!(humanise_error(msg).contains("session is not open"));
    }

    #[test]
    fn humanise_translates_canon_disconnected() {
        let msg = "Canon SDK: camera disconnected";
        assert!(humanise_error(msg).contains("disconnected"));
    }

    #[test]
    fn camera_error_display_is_human_readable() {
        let err = CameraError::DeviceNotFound("cam-1".to_string());
        assert_eq!(err.to_string(), "device not found: cam-1");
    }

    #[test]
    fn canon_error_variants_display_correctly() {
        let sdk_err = CameraError::CanonSdkError("init failed".to_string());
        assert_eq!(sdk_err.to_string(), "Canon SDK error: init failed");

        let session_err = CameraError::CanonSessionNotOpen("R5".to_string());
        assert_eq!(session_err.to_string(), "Canon session not open: R5");

        let busy_err = CameraError::CanonDeviceBusy("processing".to_string());
        assert_eq!(busy_err.to_string(), "Canon device busy: processing");
    }

    #[test]
    fn camera_error_is_clone() {
        let err = CameraError::DeviceNotFound("cam-1".to_string());
        let cloned = err.clone();
        assert_eq!(err.to_string(), cloned.to_string());
    }
}
