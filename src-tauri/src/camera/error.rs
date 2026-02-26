use thiserror::Error;

/// Camera subsystem errors.
#[derive(Debug, Error)]
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
}

/// Convenience Result alias.
pub type Result<T> = std::result::Result<T, CameraError>;
