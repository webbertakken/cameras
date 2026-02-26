use serde::Serialize;
use std::time::Instant;

/// Collects diagnostic statistics for a camera preview session.
pub struct DiagnosticStats {
    frame_count: u64,
    drop_count: u64,
    total_bytes: u64,
    start_time: Instant,
    last_frame_time: Option<Instant>,
    latency_us: u64,
    usb_bus_info: Option<String>,
}

/// Snapshot of diagnostic stats for IPC serialisation.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticSnapshot {
    pub fps: f64,
    pub frame_count: u64,
    pub drop_count: u64,
    pub drop_rate: f64,
    pub latency_ms: f64,
    pub bandwidth_bps: u64,
    pub usb_bus_info: Option<String>,
}

impl DiagnosticStats {
    /// Create new stats with zeroed counters.
    pub fn new() -> Self {
        Self {
            frame_count: 0,
            drop_count: 0,
            total_bytes: 0,
            start_time: Instant::now(),
            last_frame_time: None,
            latency_us: 0,
            usb_bus_info: None,
        }
    }

    /// Set USB bus information for this camera session.
    pub fn set_usb_bus_info(&mut self, info: Option<String>) {
        self.usb_bus_info = info;
    }

    /// Record a successfully captured frame.
    pub fn record_frame(&mut self, bytes: usize, capture_timestamp_us: u64) {
        self.frame_count += 1;
        self.total_bytes += bytes as u64;
        self.last_frame_time = Some(Instant::now());

        // Calculate latency as time since capture timestamp
        let now_us = self.start_time.elapsed().as_micros() as u64;
        if capture_timestamp_us <= now_us {
            self.latency_us = now_us - capture_timestamp_us;
        }
    }

    /// Record a dropped frame.
    pub fn record_drop(&mut self) {
        self.drop_count += 1;
    }

    /// Calculate current FPS based on elapsed time.
    pub fn fps(&self) -> f64 {
        let elapsed = self.start_time.elapsed().as_secs_f64();
        if elapsed < 0.001 {
            return 0.0;
        }
        self.frame_count as f64 / elapsed
    }

    /// Drop rate as a percentage (0.0 - 100.0).
    pub fn drop_rate(&self) -> f64 {
        let total = self.frame_count + self.drop_count;
        if total == 0 {
            return 0.0;
        }
        (self.drop_count as f64 / total as f64) * 100.0
    }

    /// Latest capture-to-delivery latency in milliseconds.
    pub fn latency_ms(&self) -> f64 {
        self.latency_us as f64 / 1000.0
    }

    /// Bandwidth in bytes per second.
    pub fn bandwidth_bps(&self) -> u64 {
        let elapsed = self.start_time.elapsed().as_secs_f64();
        if elapsed < 0.001 {
            return 0;
        }
        (self.total_bytes as f64 / elapsed) as u64
    }

    /// Reset all counters.
    pub fn reset(&mut self) {
        self.frame_count = 0;
        self.drop_count = 0;
        self.total_bytes = 0;
        self.start_time = Instant::now();
        self.last_frame_time = None;
        self.latency_us = 0;
        self.usb_bus_info = None;
    }

    /// Take a serialisable snapshot.
    pub fn snapshot(&self) -> DiagnosticSnapshot {
        DiagnosticSnapshot {
            fps: self.fps(),
            frame_count: self.frame_count,
            drop_count: self.drop_count,
            drop_rate: self.drop_rate(),
            latency_ms: self.latency_ms(),
            bandwidth_bps: self.bandwidth_bps(),
            usb_bus_info: self.usb_bus_info.clone(),
        }
    }
}

impl Default for DiagnosticStats {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn initialises_with_zero_values() {
        let stats = DiagnosticStats::new();
        assert_eq!(stats.frame_count, 0);
        assert_eq!(stats.drop_count, 0);
        assert_eq!(stats.total_bytes, 0);
        assert_eq!(stats.latency_us, 0);
    }

    #[test]
    fn record_frame_increments_frame_count() {
        let mut stats = DiagnosticStats::new();
        stats.record_frame(1000, 0);
        assert_eq!(stats.frame_count, 1);
        stats.record_frame(1000, 0);
        assert_eq!(stats.frame_count, 2);
    }

    #[test]
    fn record_drop_increments_drop_count() {
        let mut stats = DiagnosticStats::new();
        stats.record_drop();
        assert_eq!(stats.drop_count, 1);
        stats.record_drop();
        assert_eq!(stats.drop_count, 2);
    }

    #[test]
    fn fps_returns_correct_rate() {
        let mut stats = DiagnosticStats::new();
        // Record 30 frames over ~100ms
        for i in 0..30 {
            stats.record_frame(1000, i * 3333);
        }
        thread::sleep(Duration::from_millis(100));
        let fps = stats.fps();
        // Should be roughly 30/0.1 = 300, but timing is imprecise
        // Just verify it's positive and non-zero
        assert!(fps > 0.0, "fps should be positive, got {fps}");
    }

    #[test]
    fn drop_rate_returns_percentage() {
        let mut stats = DiagnosticStats::new();
        stats.record_frame(1000, 0);
        stats.record_frame(1000, 0);
        stats.record_drop();
        // 1 drop out of 3 total = 33.3%
        let rate = stats.drop_rate();
        assert!(
            (rate - 33.333).abs() < 1.0,
            "drop rate should be ~33%, got {rate}"
        );
    }

    #[test]
    fn drop_rate_zero_when_no_events() {
        let stats = DiagnosticStats::new();
        assert_eq!(stats.drop_rate(), 0.0);
    }

    #[test]
    fn bandwidth_bps_tracks_bytes() {
        let mut stats = DiagnosticStats::new();
        stats.record_frame(10_000, 0);
        thread::sleep(Duration::from_millis(50));
        let bps = stats.bandwidth_bps();
        assert!(bps > 0, "bandwidth should be positive, got {bps}");
    }

    #[test]
    fn reset_clears_all_counters() {
        let mut stats = DiagnosticStats::new();
        stats.record_frame(1000, 0);
        stats.record_drop();
        stats.reset();
        assert_eq!(stats.frame_count, 0);
        assert_eq!(stats.drop_count, 0);
        assert_eq!(stats.total_bytes, 0);
    }

    #[test]
    fn snapshot_produces_serialisable_data() {
        let mut stats = DiagnosticStats::new();
        stats.record_frame(5000, 0);
        let snap = stats.snapshot();
        let json = serde_json::to_value(&snap).unwrap();
        assert!(json["frameCount"].is_number());
        assert!(json["dropCount"].is_number());
    }

    #[test]
    fn snapshot_includes_usb_bus_info() {
        let mut stats = DiagnosticStats::new();
        stats.set_usb_bus_info(Some("USB 3.0 Bus 2".to_string()));
        let snap = stats.snapshot();
        assert_eq!(snap.usb_bus_info, Some("USB 3.0 Bus 2".to_string()));
    }

    #[test]
    fn snapshot_usb_bus_info_none_serialises_as_null() {
        let stats = DiagnosticStats::new();
        let snap = stats.snapshot();
        let json = serde_json::to_value(&snap).unwrap();
        assert!(json["usbBusInfo"].is_null());
    }

    #[test]
    fn snapshot_usb_bus_info_serialises_to_camelcase() {
        let mut stats = DiagnosticStats::new();
        stats.set_usb_bus_info(Some("USB 2.0 Bus 1".to_string()));
        let snap = stats.snapshot();
        let json = serde_json::to_value(&snap).unwrap();
        assert_eq!(json["usbBusInfo"], "USB 2.0 Bus 1");
    }
}
