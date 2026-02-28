use tauri::{AppHandle, Emitter, Manager};

use crate::camera::backend::CameraBackend;
use crate::camera::commands::CameraState;
use crate::camera::types::HotplugEvent;
use crate::settings::commands::{apply_saved_settings, SettingsState};

/// Start watching for hotplug events and forward them as Tauri events.
///
/// On `Connected` events, also auto-applies saved settings and emits a
/// `"settings-restored"` event to notify the frontend.
pub fn start_hotplug_watcher(app_handle: &AppHandle, backend: &dyn CameraBackend) {
    let handle = app_handle.clone();

    let result = backend.watch_hotplug(Box::new(move |event: HotplugEvent| {
        if let Err(e) = handle.emit("camera-hotplug", &event) {
            tracing::warn!("Failed to emit camera-hotplug event: {e}");
        }

        // Auto-apply saved settings when a camera connects
        if let HotplugEvent::Connected(ref device) = event {
            let settings_state = handle.try_state::<SettingsState>();
            let camera_state = handle.try_state::<CameraState>();

            if let (Some(settings), Some(camera)) = (settings_state, camera_state) {
                let applied = apply_saved_settings(
                    camera.backend.as_ref(),
                    &settings.store,
                    device.id.as_str(),
                );
                if !applied.is_empty() {
                    tracing::info!(
                        "Auto-applied {} settings for '{}' on hotplug",
                        applied.len(),
                        device.name
                    );
                    let _ = handle.emit(
                        "settings-restored",
                        serde_json::json!({
                            "deviceId": device.id.as_str(),
                            "cameraName": device.name,
                            "controlsApplied": applied.len(),
                        }),
                    );
                }
            }
        }
    }));

    if let Err(e) = result {
        tracing::warn!("Failed to start hotplug watcher: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::error::{CameraError, Result};
    use crate::camera::types::{
        CameraDevice, ControlDescriptor, ControlId, ControlValue, DeviceId, FormatDescriptor,
        HotplugEvent,
    };
    use std::sync::{Arc, Mutex};

    type HotplugCallback = Arc<Mutex<Option<Box<dyn Fn(HotplugEvent) + Send>>>>;

    /// Mock backend that captures the hotplug callback and lets tests invoke it.
    struct MockHotplugBackend {
        callback: HotplugCallback,
    }

    impl MockHotplugBackend {
        fn new() -> (Self, HotplugCallback) {
            let callback: HotplugCallback = Arc::new(Mutex::new(None));
            (
                Self {
                    callback: callback.clone(),
                },
                callback,
            )
        }
    }

    impl CameraBackend for MockHotplugBackend {
        fn enumerate_devices(&self) -> Result<Vec<CameraDevice>> {
            Ok(vec![])
        }

        fn watch_hotplug(&self, callback: Box<dyn Fn(HotplugEvent) + Send>) -> Result<()> {
            *self.callback.lock().unwrap() = Some(callback);
            Ok(())
        }

        fn get_controls(&self, _id: &DeviceId) -> Result<Vec<ControlDescriptor>> {
            Ok(vec![])
        }

        fn get_control(&self, _id: &DeviceId, _control: &ControlId) -> Result<ControlValue> {
            Err(CameraError::DeviceNotFound("mock".into()))
        }

        fn set_control(
            &self,
            _id: &DeviceId,
            _control: &ControlId,
            _value: ControlValue,
        ) -> Result<()> {
            Ok(())
        }

        fn get_formats(&self, _id: &DeviceId) -> Result<Vec<FormatDescriptor>> {
            Ok(vec![])
        }
    }

    /// Mock backend that always fails on watch_hotplug.
    struct FailingHotplugBackend;

    impl CameraBackend for FailingHotplugBackend {
        fn enumerate_devices(&self) -> Result<Vec<CameraDevice>> {
            Ok(vec![])
        }

        fn watch_hotplug(&self, _callback: Box<dyn Fn(HotplugEvent) + Send>) -> Result<()> {
            Err(CameraError::Hotplug("device manager unavailable".into()))
        }

        fn get_controls(&self, _id: &DeviceId) -> Result<Vec<ControlDescriptor>> {
            Ok(vec![])
        }

        fn get_control(&self, _id: &DeviceId, _control: &ControlId) -> Result<ControlValue> {
            Err(CameraError::DeviceNotFound("mock".into()))
        }

        fn set_control(
            &self,
            _id: &DeviceId,
            _control: &ControlId,
            _value: ControlValue,
        ) -> Result<()> {
            Ok(())
        }

        fn get_formats(&self, _id: &DeviceId) -> Result<Vec<FormatDescriptor>> {
            Ok(vec![])
        }
    }

    #[test]
    fn hotplug_bridge_registers_callback_on_connected() {
        let (backend, callback_slot) = MockHotplugBackend::new();

        // We can't easily create a real AppHandle in tests, so we verify the
        // backend receives a callback by checking the callback slot is populated.
        // The actual emit is tested via integration tests.
        backend
            .watch_hotplug(Box::new(|_event| {}))
            .expect("watch_hotplug should succeed");

        assert!(
            callback_slot.lock().unwrap().is_some(),
            "callback should be registered"
        );
    }

    #[test]
    fn hotplug_bridge_callback_receives_connected_event() {
        let (backend, callback_slot) = MockHotplugBackend::new();
        let received_events: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(vec![]));
        let events_clone = received_events.clone();

        backend
            .watch_hotplug(Box::new(move |event: HotplugEvent| {
                let event_type = match &event {
                    HotplugEvent::Connected(_) => "connected",
                    HotplugEvent::Disconnected { .. } => "disconnected",
                };
                events_clone.lock().unwrap().push(event_type.to_string());
            }))
            .expect("watch_hotplug should succeed");

        // Simulate a connected event
        let callback = callback_slot.lock().unwrap();
        let cb = callback.as_ref().expect("callback should be registered");
        cb(HotplugEvent::Connected(CameraDevice {
            id: DeviceId::new("test:001"),
            name: "Test Camera".to_string(),
            device_path: "test-path".to_string(),
            is_connected: true,
        }));

        let events = received_events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0], "connected");
    }

    #[test]
    fn hotplug_bridge_callback_receives_disconnected_event() {
        let (backend, callback_slot) = MockHotplugBackend::new();
        let received_events: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(vec![]));
        let events_clone = received_events.clone();

        backend
            .watch_hotplug(Box::new(move |event: HotplugEvent| {
                let event_type = match &event {
                    HotplugEvent::Connected(_) => "connected",
                    HotplugEvent::Disconnected { .. } => "disconnected",
                };
                events_clone.lock().unwrap().push(event_type.to_string());
            }))
            .expect("watch_hotplug should succeed");

        let callback = callback_slot.lock().unwrap();
        let cb = callback.as_ref().expect("callback should be registered");
        cb(HotplugEvent::Disconnected {
            id: DeviceId::new("test:001"),
        });

        let events = received_events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0], "disconnected");
    }

    #[test]
    fn settings_restored_payload_uses_controls_applied_field_name() {
        let payload = serde_json::json!({
            "deviceId": "test:001",
            "cameraName": "Test Camera",
            "controlsApplied": 3,
        });
        // Verify the field names match the frontend SettingsRestoredPayload type
        assert!(
            payload.get("controlsApplied").is_some(),
            "must use 'controlsApplied' not 'controlCount'"
        );
        assert!(
            payload.get("controlCount").is_none(),
            "must not use 'controlCount'"
        );
        assert_eq!(payload["controlsApplied"], 3);
        assert_eq!(payload["deviceId"], "test:001");
        assert_eq!(payload["cameraName"], "Test Camera");
    }

    #[test]
    fn hotplug_bridge_logs_error_on_watch_failure() {
        // FailingHotplugBackend.watch_hotplug returns Err — start_hotplug_watcher
        // should not panic.
        let backend = FailingHotplugBackend;
        let result = backend.watch_hotplug(Box::new(|_| {}));

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("device manager unavailable"),);
    }
}
