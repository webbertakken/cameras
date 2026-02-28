use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Settings for a single camera — name and control values.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CameraSettings {
    pub name: String,
    pub controls: HashMap<String, i32>,
}

/// Result of resetting a single control to its hardware default.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ResetResult {
    pub control_id: String,
    pub value: i32,
}

/// Top-level settings file structure — maps device IDs to camera settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SettingsFile {
    pub cameras: HashMap<String, CameraSettings>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_result_serialises_to_camel_case_json() {
        let result = ResetResult {
            control_id: "brightness".to_string(),
            value: 128,
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["controlId"], "brightness");
        assert_eq!(json["value"], 128);
        // Must not use snake_case field names
        assert!(json.get("control_id").is_none());
    }

    #[test]
    fn reset_result_round_trips_through_json() {
        let result = ResetResult {
            control_id: "contrast".to_string(),
            value: 50,
        };
        let json = serde_json::to_string(&result).unwrap();
        let restored: ResetResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, restored);
    }

    #[test]
    fn camera_settings_default_is_empty() {
        let settings = CameraSettings::default();
        assert_eq!(settings.name, "");
        assert!(settings.controls.is_empty());
    }

    #[test]
    fn settings_file_serialises_to_json() {
        let mut controls = HashMap::new();
        controls.insert("brightness".to_string(), 150);
        controls.insert("contrast".to_string(), 60);

        let mut cameras = HashMap::new();
        cameras.insert(
            "046d:085e:serial".to_string(),
            CameraSettings {
                name: "Logitech BRIO".to_string(),
                controls,
            },
        );

        let file = SettingsFile { cameras };
        let json = serde_json::to_value(&file).unwrap();

        assert!(json["cameras"]["046d:085e:serial"].is_object());
        assert_eq!(json["cameras"]["046d:085e:serial"]["name"], "Logitech BRIO");
        assert_eq!(
            json["cameras"]["046d:085e:serial"]["controls"]["brightness"],
            150
        );
        assert_eq!(
            json["cameras"]["046d:085e:serial"]["controls"]["contrast"],
            60
        );
    }

    #[test]
    fn settings_file_deserialises_from_json() {
        let json = r#"{
            "cameras": {
                "device-001": {
                    "name": "Test Camera",
                    "controls": {
                        "brightness": 200,
                        "saturation": 100
                    }
                }
            }
        }"#;

        let file: SettingsFile = serde_json::from_str(json).unwrap();
        assert_eq!(file.cameras.len(), 1);

        let cam = &file.cameras["device-001"];
        assert_eq!(cam.name, "Test Camera");
        assert_eq!(cam.controls["brightness"], 200);
        assert_eq!(cam.controls["saturation"], 100);
    }

    #[test]
    fn settings_file_round_trips_through_json() {
        let mut controls = HashMap::new();
        controls.insert("brightness".to_string(), 128);

        let mut cameras = HashMap::new();
        cameras.insert(
            "test-id".to_string(),
            CameraSettings {
                name: "Camera".to_string(),
                controls,
            },
        );

        let original = SettingsFile { cameras };
        let json = serde_json::to_string(&original).unwrap();
        let restored: SettingsFile = serde_json::from_str(&json).unwrap();

        assert_eq!(original, restored);
    }

    #[test]
    fn settings_file_handles_multiple_cameras() {
        let mut cameras = HashMap::new();
        cameras.insert(
            "cam-1".to_string(),
            CameraSettings {
                name: "Camera One".to_string(),
                controls: {
                    let mut c = HashMap::new();
                    c.insert("brightness".to_string(), 100);
                    c
                },
            },
        );
        cameras.insert(
            "cam-2".to_string(),
            CameraSettings {
                name: "Camera Two".to_string(),
                controls: {
                    let mut c = HashMap::new();
                    c.insert("contrast".to_string(), 50);
                    c
                },
            },
        );

        let file = SettingsFile { cameras };
        let json = serde_json::to_string(&file).unwrap();
        let restored: SettingsFile = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.cameras.len(), 2);
        assert_eq!(restored.cameras["cam-1"].name, "Camera One");
        assert_eq!(restored.cameras["cam-2"].name, "Camera Two");
    }
}
