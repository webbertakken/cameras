use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;
use tokio::sync::Notify;

use crate::settings::types::SettingsFile;

/// Persistent settings store with debounced saving.
pub struct SettingsStore {
    path: PathBuf,
    data: Mutex<SettingsFile>,
    save_notify: Notify,
    is_dirty: AtomicBool,
}

impl SettingsStore {
    /// Create a new store, loading from disk if the file exists.
    pub fn new(path: PathBuf) -> Self {
        let data = Self::load(&path).unwrap_or_default();
        Self {
            path,
            data: Mutex::new(data),
            save_notify: Notify::new(),
            is_dirty: AtomicBool::new(false),
        }
    }

    /// Load settings from a JSON file, returning default on missing file.
    pub fn load(path: &std::path::Path) -> Result<SettingsFile, String> {
        if !path.exists() {
            return Ok(SettingsFile::default());
        }
        let contents = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        serde_json::from_str(&contents).map_err(|e| e.to_string())
    }

    /// Save current settings to disk atomically (write .tmp then rename).
    pub fn save(&self) -> Result<(), String> {
        let data = self.data.lock().clone();
        let json = serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?;

        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        let tmp_path = self.path.with_extension("json.tmp");
        std::fs::write(&tmp_path, &json).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp_path, &self.path).map_err(|e| e.to_string())?;

        Ok(())
    }

    /// Get saved settings for a camera by device ID.
    pub fn get_camera(&self, device_id: &str) -> Option<crate::settings::types::CameraSettings> {
        self.data.lock().cameras.get(device_id).cloned()
    }

    /// Set a control value, creating the camera entry if needed.
    /// Triggers a debounced save.
    pub fn set_control(&self, device_id: &str, camera_name: &str, control_id: &str, value: i32) {
        {
            let mut data = self.data.lock();
            let entry = data.cameras.entry(device_id.to_string()).or_default();
            entry.name = camera_name.to_string();
            entry.controls.insert(control_id.to_string(), value);
        }
        self.is_dirty.store(true, Ordering::Release);
        self.save_notify.notify_one();
    }

    /// Remove all saved settings for a camera.
    pub fn remove_camera(&self, device_id: &str) {
        self.data.lock().cameras.remove(device_id);
        self.is_dirty.store(true, Ordering::Release);
        self.save_notify.notify_one();
    }

    /// Start the debounce task — waits for dirty notification, sleeps 500ms, then saves.
    ///
    /// Uses an `AtomicBool` dirty flag to avoid losing notifications that arrive
    /// between `save()` completing and `notified().await` re-registering.
    pub fn start_debounce_task(self: &Arc<Self>) {
        let store = Arc::clone(self);
        tauri::async_runtime::spawn(async move {
            loop {
                store.save_notify.notified().await;
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                if store.is_dirty.swap(false, Ordering::AcqRel) {
                    if let Err(e) = store.save() {
                        tracing::warn!("Failed to save settings: {e}");
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::types::CameraSettings;
    use std::collections::HashMap;
    use tempfile::TempDir;

    /// Helper: create a store backed by a temp directory.
    fn temp_store() -> (SettingsStore, TempDir) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("cameras.json");
        let store = SettingsStore::new(path);
        (store, dir)
    }

    // --- File I/O tests (Step 3) ---

    #[test]
    fn load_returns_default_when_file_missing() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nonexistent.json");
        let result = SettingsStore::load(&path).unwrap();
        assert_eq!(result, SettingsFile::default());
    }

    #[test]
    fn load_parses_valid_json_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("cameras.json");
        let json = r#"{"cameras":{"dev-1":{"name":"Cam","controls":{"brightness":100}}}}"#;
        std::fs::write(&path, json).unwrap();

        let result = SettingsStore::load(&path).unwrap();
        assert_eq!(result.cameras.len(), 1);
        assert_eq!(result.cameras["dev-1"].name, "Cam");
        assert_eq!(result.cameras["dev-1"].controls["brightness"], 100);
    }

    #[test]
    fn load_returns_error_for_invalid_json() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("cameras.json");
        std::fs::write(&path, "not valid json!!!").unwrap();

        let result = SettingsStore::load(&path);
        assert!(result.is_err());
    }

    #[test]
    fn save_creates_file_on_disk() {
        let (store, dir) = temp_store();
        store.set_control("dev-1", "Camera", "brightness", 150);
        store.save().unwrap();

        let path = dir.path().join("cameras.json");
        assert!(path.exists());
    }

    #[test]
    fn save_writes_valid_json() {
        let (store, dir) = temp_store();
        store.set_control("dev-1", "Camera", "brightness", 150);
        store.save().unwrap();

        let path = dir.path().join("cameras.json");
        let contents = std::fs::read_to_string(&path).unwrap();
        let parsed: SettingsFile = serde_json::from_str(&contents).unwrap();
        assert_eq!(parsed.cameras["dev-1"].controls["brightness"], 150);
    }

    #[test]
    fn save_round_trips_through_load() {
        let (store, dir) = temp_store();
        store.set_control("dev-1", "Camera One", "brightness", 200);
        store.set_control("dev-2", "Camera Two", "contrast", 50);
        store.save().unwrap();

        let path = dir.path().join("cameras.json");
        let loaded = SettingsStore::load(&path).unwrap();
        assert_eq!(loaded.cameras.len(), 2);
        assert_eq!(loaded.cameras["dev-1"].name, "Camera One");
        assert_eq!(loaded.cameras["dev-1"].controls["brightness"], 200);
        assert_eq!(loaded.cameras["dev-2"].name, "Camera Two");
        assert_eq!(loaded.cameras["dev-2"].controls["contrast"], 50);
    }

    #[test]
    fn save_creates_parent_directories() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nested").join("deep").join("cameras.json");
        let store = SettingsStore::new(path.clone());
        store.set_control("dev-1", "Camera", "brightness", 100);
        store.save().unwrap();

        assert!(path.exists());
    }

    #[test]
    fn save_is_atomic() {
        let (store, dir) = temp_store();
        store.set_control("dev-1", "Camera", "brightness", 100);
        store.save().unwrap();

        // After a successful save, no .tmp file should remain
        let tmp_path = dir.path().join("cameras.json.tmp");
        assert!(
            !tmp_path.exists(),
            ".tmp file should be cleaned up after rename"
        );
    }

    #[test]
    fn new_loads_existing_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("cameras.json");

        // Write a settings file manually
        let mut controls = HashMap::new();
        controls.insert("brightness".to_string(), 200);
        let mut cameras = HashMap::new();
        cameras.insert(
            "dev-1".to_string(),
            CameraSettings {
                name: "Pre-existing".to_string(),
                controls,
            },
        );
        let file = SettingsFile { cameras };
        std::fs::write(&path, serde_json::to_string(&file).unwrap()).unwrap();

        // SettingsStore::new should load it
        let store = SettingsStore::new(path);
        let cam = store.get_camera("dev-1").unwrap();
        assert_eq!(cam.name, "Pre-existing");
        assert_eq!(cam.controls["brightness"], 200);
    }

    // --- In-memory operation tests (Step 4) ---

    #[test]
    fn get_camera_returns_none_for_unknown() {
        let (store, _dir) = temp_store();
        assert!(store.get_camera("nonexistent").is_none());
    }

    #[test]
    fn get_camera_returns_saved_settings() {
        let (store, _dir) = temp_store();
        store.set_control("dev-1", "Camera", "brightness", 128);
        let cam = store.get_camera("dev-1").unwrap();
        assert_eq!(cam.name, "Camera");
        assert_eq!(cam.controls["brightness"], 128);
    }

    #[test]
    fn set_control_creates_new_camera_entry() {
        let (store, _dir) = temp_store();
        assert!(store.get_camera("new-device").is_none());
        store.set_control("new-device", "New Camera", "exposure", 50);
        let cam = store.get_camera("new-device").unwrap();
        assert_eq!(cam.name, "New Camera");
        assert_eq!(cam.controls["exposure"], 50);
    }

    #[test]
    fn set_control_updates_existing_entry() {
        let (store, _dir) = temp_store();
        store.set_control("dev-1", "Camera", "brightness", 100);
        store.set_control("dev-1", "Camera", "brightness", 200);
        let cam = store.get_camera("dev-1").unwrap();
        assert_eq!(cam.controls["brightness"], 200);
    }

    #[test]
    fn set_control_preserves_other_cameras() {
        let (store, _dir) = temp_store();
        store.set_control("dev-1", "Camera One", "brightness", 100);
        store.set_control("dev-2", "Camera Two", "contrast", 50);

        // Updating dev-1 should not affect dev-2
        store.set_control("dev-1", "Camera One", "brightness", 200);

        let cam2 = store.get_camera("dev-2").unwrap();
        assert_eq!(cam2.name, "Camera Two");
        assert_eq!(cam2.controls["contrast"], 50);
    }

    #[test]
    fn set_control_preserves_other_controls() {
        let (store, _dir) = temp_store();
        store.set_control("dev-1", "Camera", "brightness", 100);
        store.set_control("dev-1", "Camera", "contrast", 50);

        let cam = store.get_camera("dev-1").unwrap();
        assert_eq!(cam.controls["brightness"], 100);
        assert_eq!(cam.controls["contrast"], 50);
    }

    #[test]
    fn set_control_updates_camera_name() {
        let (store, _dir) = temp_store();
        store.set_control("dev-1", "Old Name", "brightness", 100);
        store.set_control("dev-1", "New Name", "brightness", 100);

        let cam = store.get_camera("dev-1").unwrap();
        assert_eq!(cam.name, "New Name");
    }

    #[test]
    fn remove_camera_deletes_entry() {
        let (store, _dir) = temp_store();
        store.set_control("dev-1", "Camera", "brightness", 100);
        assert!(store.get_camera("dev-1").is_some());

        store.remove_camera("dev-1");
        assert!(store.get_camera("dev-1").is_none());
    }

    #[test]
    fn remove_camera_is_idempotent() {
        let (store, _dir) = temp_store();
        store.remove_camera("nonexistent"); // should not panic
        store.remove_camera("nonexistent"); // still should not panic
    }
}
