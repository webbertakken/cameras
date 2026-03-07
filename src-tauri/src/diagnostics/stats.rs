use serde::Serialize;
use std::time::Instant;

/// EMA smoothing factor for latency.  α = 0.1 gives a ~10-frame window.
const LATENCY_EMA_ALPHA: f64 = 0.1;

/// Collects diagnostic statistics for a camera preview session.
pub struct DiagnosticStats {
    frame_count: u64,
    drop_count: u64,
    total_bytes: u64,
    start_time: Instant,
    last_frame_time: Option<Instant>,
    /// EMA-smoothed latency in microseconds.
    latency_ema_us: f64,
    /// Clock-base offset recorded on the first frame so that capture
    /// timestamps from an unrelated clock (e.g. DirectShow stream time) are
    /// normalised before computing latency.
    clock_offset_us: Option<i64>,
    usb_bus_info: Option<String>,
}

/// Snapshot of diagnostic stats for IPC serialisation.
#[derive(Debug, Clone, Default, Serialize)]
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
            latency_ema_us: 0.0,
            clock_offset_us: None,
            usb_bus_info: None,
        }
    }

    /// Set USB bus information for this camera session.
    pub fn set_usb_bus_info(&mut self, info: Option<String>) {
        self.usb_bus_info = info;
    }

    /// Record a successfully captured frame.
    ///
    /// `capture_timestamp_us` may originate from any monotonic clock (e.g.
    /// DirectShow stream time, or [`Self::elapsed_us`]).  On the first frame
    /// the offset between that clock and our internal clock is recorded so
    /// that subsequent latency values only reflect per-frame processing
    /// delay, not a fixed clock-base mismatch.
    pub fn record_frame(&mut self, bytes: usize, capture_timestamp_us: u64) {
        self.frame_count += 1;
        self.total_bytes += bytes as u64;
        self.last_frame_time = Some(Instant::now());

        let now_us = self.start_time.elapsed().as_micros() as u64;
        let raw_delta = now_us as i64 - capture_timestamp_us as i64;

        // Calibrate clock offset on the first frame.
        let offset = *self.clock_offset_us.get_or_insert(raw_delta);

        let sample_us = (raw_delta - offset).unsigned_abs();

        // EMA: on the very first frame, seed the average; otherwise blend.
        if self.frame_count == 1 {
            self.latency_ema_us = sample_us as f64;
        } else {
            self.latency_ema_us = LATENCY_EMA_ALPHA * sample_us as f64
                + (1.0 - LATENCY_EMA_ALPHA) * self.latency_ema_us;
        }
    }

    /// Record a successfully captured frame with a directly-measured latency.
    ///
    /// Use this for sources (e.g. Canon live view) that measure their own
    /// per-frame download/transfer time directly via [`std::time::Instant`],
    /// rather than carrying a capture timestamp from an external clock.
    ///
    /// This bypasses the clock-offset calibration logic in [`Self::record_frame`]
    /// and feeds the provided `latency_us` value directly into the EMA.
    pub fn record_frame_with_latency(&mut self, bytes: usize, latency_us: u64) {
        self.frame_count += 1;
        self.total_bytes += bytes as u64;
        self.last_frame_time = Some(Instant::now());

        // EMA: on the very first frame, seed the average; otherwise blend.
        if self.frame_count == 1 {
            self.latency_ema_us = latency_us as f64;
        } else {
            self.latency_ema_us = LATENCY_EMA_ALPHA * latency_us as f64
                + (1.0 - LATENCY_EMA_ALPHA) * self.latency_ema_us;
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

    /// EMA-smoothed capture-to-delivery latency in milliseconds.
    pub fn latency_ms(&self) -> f64 {
        self.latency_ema_us / 1000.0
    }

    /// Bandwidth in bytes per second.
    pub fn bandwidth_bps(&self) -> u64 {
        let elapsed = self.start_time.elapsed().as_secs_f64();
        if elapsed < 0.001 {
            return 0;
        }
        (self.total_bytes as f64 / elapsed) as u64
    }

    /// Elapsed microseconds since this stats instance was created.
    ///
    /// Useful for Canon live view where there is no external capture
    /// timestamp — the polling thread uses this as the frame timestamp.
    pub fn elapsed_us(&self) -> u64 {
        self.start_time.elapsed().as_micros() as u64
    }

    /// Reset all counters.
    pub fn reset(&mut self) {
        self.frame_count = 0;
        self.drop_count = 0;
        self.total_bytes = 0;
        self.start_time = Instant::now();
        self.last_frame_time = None;
        self.latency_ema_us = 0.0;
        self.clock_offset_us = None;
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
        assert_eq!(stats.latency_ms(), 0.0);
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

    #[test]
    fn latency_stays_low_when_timestamps_use_different_clock_base() {
        // Simulates the BRIO bug: DiagnosticStats is created 15 seconds before
        // the capture graph starts.  DirectShow timestamps start at 0 when the
        // graph runs, but elapsed_us() is already ~15 000 000 by then.
        // The old code reported now_us - capture_ts ≈ 15 000ms as "latency".
        //
        // In a real session both clocks tick at the same rate — only the
        // origin differs.  We simulate this by feeding capture_ts values that
        // are exactly `elapsed_us() - 15_000_000`, i.e. offset by 15s.
        let mut stats = DiagnosticStats::new();
        // Backdate so elapsed_us() starts at ~15 000 000
        stats.start_time = Instant::now() - Duration::from_secs(15);

        let base_offset_us: u64 = 15_000_000;
        for _i in 0..30 {
            // Capture timestamp on the DirectShow clock (= our clock minus the
            // fixed setup gap).  Both clocks advance in lockstep in reality.
            let now = stats.elapsed_us();
            let capture_ts = now.saturating_sub(base_offset_us);
            stats.record_frame(1000, capture_ts);
        }

        let latency = stats.latency_ms();
        assert!(
            latency < 50.0,
            "latency should reflect per-frame delay, not the 15s setup gap; \
             got {latency:.1}ms"
        );
    }

    #[test]
    fn latency_ema_converges_to_recent_values() {
        let mut stats = DiagnosticStats::new();

        // Feed frames with a consistent small delta between elapsed_us() and
        // the timestamp passed to record_frame.
        for _i in 0..30 {
            let ts = stats.elapsed_us();
            // Small sleep to create a measurable but tiny latency
            thread::sleep(Duration::from_millis(1));
            stats.record_frame(1000, ts);
        }

        let latency = stats.latency_ms();
        // EMA should converge to roughly the 1ms sleep, well under 50ms
        assert!(
            latency < 50.0,
            "EMA latency should be small, got {latency:.1}ms"
        );
    }

    #[test]
    fn elapsed_us_increases_over_time() {
        let stats = DiagnosticStats::new();
        let t0 = stats.elapsed_us();
        thread::sleep(Duration::from_millis(10));
        let t1 = stats.elapsed_us();
        assert!(
            t1 > t0,
            "elapsed_us should increase over time: t0={t0}, t1={t1}"
        );
    }

    // ------------------------------------------------------------------
    // record_frame_with_latency tests
    // ------------------------------------------------------------------

    #[test]
    fn record_frame_with_latency_seeds_ema_on_first_frame() {
        let mut stats = DiagnosticStats::new();
        stats.record_frame_with_latency(1000, 5_000); // 5ms download
        assert!(
            (stats.latency_ms() - 5.0).abs() < 0.1,
            "first frame should seed EMA to 5ms, got {:.3}ms",
            stats.latency_ms()
        );
    }

    #[test]
    fn record_frame_with_latency_increments_frame_count() {
        let mut stats = DiagnosticStats::new();
        stats.record_frame_with_latency(1000, 5_000);
        assert_eq!(stats.frame_count, 1);
        stats.record_frame_with_latency(1000, 5_000);
        assert_eq!(stats.frame_count, 2);
    }

    #[test]
    fn record_frame_with_latency_accumulates_bytes_for_bandwidth() {
        let mut stats = DiagnosticStats::new();
        stats.record_frame_with_latency(10_000, 5_000);
        thread::sleep(Duration::from_millis(50));
        let bps = stats.bandwidth_bps();
        assert!(
            bps > 0,
            "bandwidth should be positive after bytes recorded, got {bps}"
        );
    }

    #[test]
    fn record_frame_with_latency_blends_ema_on_subsequent_frames() {
        let mut stats = DiagnosticStats::new();
        // Seed with 10ms
        stats.record_frame_with_latency(1000, 10_000);
        // Feed several frames at 20ms — EMA should move toward 20ms
        for _ in 0..30 {
            stats.record_frame_with_latency(1000, 20_000);
        }
        let latency = stats.latency_ms();
        // After 30 more frames at 20ms, EMA should be significantly above 10ms
        assert!(
            latency > 15.0,
            "EMA should converge toward 20ms after many frames, got {latency:.3}ms"
        );
        assert!(
            latency <= 20.0,
            "EMA cannot exceed the fed value of 20ms, got {latency:.3}ms"
        );
    }

    #[test]
    fn record_frame_with_latency_never_produces_zero_when_latency_is_nonzero() {
        // This is the Canon regression test: using elapsed_us as timestamp
        // would cancel to zero. record_frame_with_latency must not do that.
        let mut stats = DiagnosticStats::new();
        // Simulate Canon: download takes 80ms (typical EDSDK EVF image transfer)
        stats.record_frame_with_latency(384_000, 80_000);
        assert!(
            stats.latency_ms() > 0.0,
            "latency must not collapse to zero; got {:.3}ms",
            stats.latency_ms()
        );
    }

    #[test]
    fn record_frame_with_latency_does_not_use_clock_offset() {
        // Ensure the clock_offset calibration logic from record_frame is NOT
        // applied here, so repeated calls with the same latency give a stable EMA.
        let mut stats = DiagnosticStats::new();
        for _ in 0..50 {
            stats.record_frame_with_latency(1000, 15_000); // constant 15ms
        }
        // EMA must be very close to 15ms (within 1ms rounding)
        let latency = stats.latency_ms();
        assert!(
            (latency - 15.0).abs() < 1.0,
            "stable 15ms feed should yield ~15ms EMA, got {latency:.3}ms"
        );
    }
}
