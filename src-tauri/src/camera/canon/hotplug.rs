//! Canon hotplug detection via periodic re-enumeration.
//!
//! EDSDK doesn't have a reliable push-based connection notification,
//! so we poll for camera list changes at regular intervals.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use crate::camera::canon::api::EdsSdkApi;
use crate::camera::canon::discovery::discover_cameras;
use crate::camera::types::{DeviceId, HotplugEvent};

/// Default re-enumeration interval.
const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(3);

/// Hotplug watcher for Canon cameras.
pub struct CanonHotplugWatcher {
    running: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl CanonHotplugWatcher {
    /// Start watching for Canon camera connections/disconnections.
    pub fn start<S: EdsSdkApi + 'static>(
        sdk: Arc<S>,
        callback: Box<dyn Fn(HotplugEvent) + Send>,
    ) -> Self {
        Self::start_with_interval(sdk, callback, DEFAULT_POLL_INTERVAL)
    }

    /// Start with a custom polling interval (useful for testing).
    pub fn start_with_interval<S: EdsSdkApi + 'static>(
        sdk: Arc<S>,
        callback: Box<dyn Fn(HotplugEvent) + Send>,
        interval: Duration,
    ) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = Arc::clone(&running);

        let thread = std::thread::Builder::new()
            .name("canon-hotplug".to_string())
            .spawn(move || {
                poll_connections(&*sdk, &callback, &running_clone, interval);
            })
            .expect("failed to spawn Canon hotplug thread");

        Self {
            running,
            thread: Some(thread),
        }
    }

    /// Stop the hotplug watcher.
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

/// Polling loop that detects connection/disconnection events.
fn poll_connections<S: EdsSdkApi>(
    sdk: &S,
    callback: &dyn Fn(HotplugEvent),
    running: &AtomicBool,
    interval: Duration,
) {
    let mut known_ids: HashSet<DeviceId> = HashSet::new();

    // Initial enumeration
    if let Ok(cameras) = discover_cameras(sdk) {
        for (_, device) in &cameras {
            known_ids.insert(device.id.clone());
        }
    }

    // Also process EDSDK events each cycle
    let _ = sdk.get_event();

    while running.load(Ordering::Relaxed) {
        std::thread::sleep(interval);

        if !running.load(Ordering::Relaxed) {
            break;
        }

        // Process pending EDSDK events
        let _ = sdk.get_event();

        let current = match discover_cameras(sdk) {
            Ok(cameras) => cameras,
            Err(e) => {
                tracing::debug!("Canon re-enumeration failed: {e}");
                continue;
            }
        };

        let current_ids: HashSet<DeviceId> = current.iter().map(|(_, d)| d.id.clone()).collect();

        // Detect new connections
        for (_, device) in &current {
            if !known_ids.contains(&device.id) {
                tracing::info!("Canon camera connected: {}", device.name);
                callback(HotplugEvent::Connected(device.clone()));
            }
        }

        // Detect disconnections
        for id in &known_ids {
            if !current_ids.contains(id) {
                tracing::info!("Canon camera disconnected: {id}");
                callback(HotplugEvent::Disconnected { id: id.clone() });
            }
        }

        known_ids = current_ids;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::canon::mock::MockEdsSdk;
    use std::sync::Mutex;

    #[test]
    fn detects_initial_cameras() {
        let events: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let events_clone = Arc::clone(&events);

        let mock = Arc::new(MockEdsSdk::new().with_camera("Canon EOS R5", Some("SER001")));

        let mut watcher = CanonHotplugWatcher::start_with_interval(
            mock,
            Box::new(move |_event| {
                events_clone.lock().unwrap().push("event".to_string());
            }),
            Duration::from_millis(10),
        );

        // Wait a bit then stop
        std::thread::sleep(Duration::from_millis(50));
        watcher.stop();

        // Initial cameras should NOT generate connect events
        // (they're already known at startup)
        let captured = events.lock().unwrap();
        assert!(
            captured.is_empty(),
            "initial cameras should not fire events: {captured:?}"
        );
    }

    #[test]
    fn watcher_stops_cleanly() {
        let mock = Arc::new(MockEdsSdk::new());
        let mut watcher = CanonHotplugWatcher::start_with_interval(
            mock,
            Box::new(|_| {}),
            Duration::from_millis(10),
        );
        watcher.stop();
        // Should not hang or panic
    }

    #[test]
    fn processes_edsdk_events() {
        let mock = Arc::new(MockEdsSdk::new());
        let mock_ref = Arc::clone(&mock);

        let mut watcher = CanonHotplugWatcher::start_with_interval(
            mock_ref,
            Box::new(|_| {}),
            Duration::from_millis(10),
        );

        std::thread::sleep(Duration::from_millis(50));
        watcher.stop();

        assert!(
            mock.events_processed() > 0,
            "should have processed EDSDK events"
        );
    }
}
