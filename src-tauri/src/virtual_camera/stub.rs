use super::VirtualCameraSink;

/// Fallback sink for unsupported platforms.
///
/// Always returns an error on `start()` — used on macOS and other platforms
/// where virtual camera output is not yet implemented.
pub struct StubSink;

impl VirtualCameraSink for StubSink {
    fn start(&mut self) -> Result<(), String> {
        Err("virtual camera not yet supported on this platform".to_string())
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
    fn stub_sink_returns_not_supported() {
        let mut sink = StubSink;
        let err = sink.start().unwrap_err();
        assert!(
            err.contains("not yet supported"),
            "expected 'not yet supported' message, got: {err}"
        );
    }

    #[test]
    fn stub_sink_stop_is_idempotent() {
        let mut sink = StubSink;
        assert!(sink.stop().is_ok());
        assert!(sink.stop().is_ok());
    }

    #[test]
    fn stub_sink_is_not_running() {
        let sink = StubSink;
        assert!(!sink.is_running());
    }
}
