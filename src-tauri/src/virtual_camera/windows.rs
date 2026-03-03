use super::VirtualCameraSink;

/// DirectShow-based virtual camera sink for Windows.
///
/// Placeholder — will push frames to a DirectShow virtual camera source
/// filter once the full implementation is complete.
pub struct DirectShowVirtualCamera;

impl VirtualCameraSink for DirectShowVirtualCamera {
    fn start(&mut self) -> Result<(), String> {
        Err("DirectShow virtual camera not yet implemented".to_string())
    }

    fn stop(&mut self) -> Result<(), String> {
        Ok(())
    }

    fn is_running(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directshow_sink_returns_not_implemented() {
        let mut sink = DirectShowVirtualCamera;
        let err = sink.start().unwrap_err();
        assert!(
            err.contains("not yet implemented"),
            "expected 'not yet implemented' message, got: {err}"
        );
    }

    #[test]
    fn directshow_sink_stop_is_idempotent() {
        let mut sink = DirectShowVirtualCamera;
        assert!(sink.stop().is_ok());
        assert!(sink.stop().is_ok());
    }

    #[test]
    fn directshow_sink_is_not_running() {
        let sink = DirectShowVirtualCamera;
        assert!(!sink.is_running());
    }
}
