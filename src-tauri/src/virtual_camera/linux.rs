use super::VirtualCameraSink;

/// v4l2loopback-based virtual camera sink for Linux.
///
/// Placeholder — will write frames to a v4l2loopback device once implemented.
pub struct V4l2LoopbackSink;

impl VirtualCameraSink for V4l2LoopbackSink {
    fn start(&mut self) -> Result<(), String> {
        Err("v4l2loopback virtual camera not yet implemented".to_string())
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
    fn v4l2_sink_returns_not_implemented() {
        let mut sink = V4l2LoopbackSink;
        let err = sink.start().unwrap_err();
        assert!(
            err.contains("not yet implemented"),
            "expected 'not yet implemented' message, got: {err}"
        );
    }

    #[test]
    fn v4l2_sink_stop_is_idempotent() {
        let mut sink = V4l2LoopbackSink;
        assert!(sink.stop().is_ok());
        assert!(sink.stop().is_ok());
    }

    #[test]
    fn v4l2_sink_is_not_running() {
        let sink = V4l2LoopbackSink;
        assert!(!sink.is_running());
    }
}
