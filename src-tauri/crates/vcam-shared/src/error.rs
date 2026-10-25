use std::fmt;

/// Errors that can occur during shared memory operations.
#[derive(Debug)]
pub enum Error {
    /// Frame data size does not match expected frame_size.
    FrameSizeMismatch { expected: usize, actual: usize },
    /// Shared memory region has invalid magic value.
    InvalidMagic(u32),
    /// Shared memory region has incompatible version.
    VersionMismatch { expected: u32, actual: u32 },
    /// A Windows API call failed.
    #[cfg(windows)]
    Windows(windows::core::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FrameSizeMismatch { expected, actual } => {
                write!(
                    f,
                    "frame size mismatch: expected {expected} bytes, got {actual}"
                )
            }
            Self::InvalidMagic(m) => write!(f, "invalid shared memory magic: 0x{m:08X}"),
            Self::VersionMismatch { expected, actual } => {
                write!(f, "version mismatch: expected {expected}, got {actual}")
            }
            #[cfg(windows)]
            Self::Windows(e) => write!(f, "Windows API error: {e}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            #[cfg(windows)]
            Self::Windows(e) => Some(e),
            _ => None,
        }
    }
}

#[cfg(windows)]
impl From<windows::core::Error> for Error {
    fn from(e: windows::core::Error) -> Self {
        Self::Windows(e)
    }
}
