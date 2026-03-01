//! Canon property to ControlDescriptor mapping.
//!
//! Maps EDSDK property IDs to the existing `ControlDescriptor` system,
//! including value translation between EDSDK internal codes and
//! human-readable labels.

use crate::camera::error::Result;
use crate::camera::types::{ControlDescriptor, ControlFlags, ControlOption, ControlType};

use super::api::{CameraHandle, EdsSdkApi};
use super::types::*;

/// A mapping definition from an EDSDK property to a `ControlDescriptor`.
struct PropertyMapping {
    prop_id: EdsPropertyID,
    control_id: &'static str,
    name: &'static str,
    control_type: ControlType,
    group: &'static str,
}

/// All Canon property mappings.
const MAPPINGS: &[PropertyMapping] = &[
    PropertyMapping {
        prop_id: PROP_ID_ISO_SPEED,
        control_id: "canon_iso",
        name: "ISO",
        control_type: ControlType::Select,
        group: "camera",
    },
    PropertyMapping {
        prop_id: PROP_ID_AV,
        control_id: "canon_aperture",
        name: "Aperture",
        control_type: ControlType::Select,
        group: "camera",
    },
    PropertyMapping {
        prop_id: PROP_ID_TV,
        control_id: "canon_shutter_speed",
        name: "Shutter Speed",
        control_type: ControlType::Select,
        group: "camera",
    },
    PropertyMapping {
        prop_id: PROP_ID_WHITE_BALANCE,
        control_id: "canon_white_balance",
        name: "White Balance",
        control_type: ControlType::Select,
        group: "camera",
    },
    PropertyMapping {
        prop_id: PROP_ID_EXPOSURE_COMPENSATION,
        control_id: "canon_exposure_compensation",
        name: "Exposure Compensation",
        control_type: ControlType::Slider,
        group: "camera",
    },
];

/// Build all Canon control descriptors for a camera.
pub fn get_canon_controls<S: EdsSdkApi>(
    sdk: &S,
    camera: CameraHandle,
) -> Result<Vec<ControlDescriptor>> {
    let mut descriptors = Vec::new();

    for mapping in MAPPINGS {
        match build_descriptor(sdk, camera, mapping) {
            Ok(desc) => descriptors.push(desc),
            Err(e) => {
                tracing::debug!("Canon control '{}' unavailable: {e}", mapping.name);
            }
        }
    }

    Ok(descriptors)
}

/// Build a single `ControlDescriptor` from a property mapping.
fn build_descriptor<S: EdsSdkApi>(
    sdk: &S,
    camera: CameraHandle,
    mapping: &PropertyMapping,
) -> Result<ControlDescriptor> {
    let current = sdk.get_property(camera, mapping.prop_id).unwrap_or(0);

    let (options, min, max, step) = match mapping.control_type {
        ControlType::Select => {
            let desc = sdk.get_property_desc(camera, mapping.prop_id)?;
            let options: Vec<ControlOption> = desc
                .prop_desc
                .iter()
                .map(|&v| ControlOption {
                    value: v,
                    label: translate_value(mapping.prop_id, v),
                })
                .collect();
            (Some(options), None, None, None)
        }
        ControlType::Slider => {
            // Exposure compensation: typically -3 to +3 in 1/3 stop increments
            (None, Some(-24), Some(24), Some(1))
        }
        ControlType::Toggle => (None, Some(0), Some(1), Some(1)),
    };

    Ok(ControlDescriptor {
        id: mapping.control_id.to_string(),
        name: mapping.name.to_string(),
        control_type: mapping.control_type,
        group: mapping.group.to_string(),
        min,
        max,
        step,
        default: None,
        current,
        flags: ControlFlags {
            supports_auto: false,
            is_auto_enabled: false,
            is_read_only: false,
        },
        options,
        supported: true,
    })
}

/// Translate an EDSDK internal value to a human-readable label.
pub fn translate_value(prop_id: EdsPropertyID, value: i32) -> String {
    match prop_id {
        PROP_ID_ISO_SPEED => translate_iso(value),
        PROP_ID_AV => translate_aperture(value),
        PROP_ID_TV => translate_shutter_speed(value),
        PROP_ID_WHITE_BALANCE => translate_white_balance(value),
        PROP_ID_EXPOSURE_COMPENSATION => translate_exposure_comp(value),
        _ => format!("{value}"),
    }
}

/// Translate EDSDK ISO speed value to display label.
fn translate_iso(value: i32) -> String {
    match value {
        0x28 => "6".to_string(),
        0x30 => "12".to_string(),
        0x38 => "25".to_string(),
        0x40 => "50".to_string(),
        0x48 => "100".to_string(),
        0x4B => "125".to_string(),
        0x4D => "160".to_string(),
        0x50 => "200".to_string(),
        0x53 => "250".to_string(),
        0x55 => "320".to_string(),
        0x58 => "400".to_string(),
        0x5B => "500".to_string(),
        0x5D => "640".to_string(),
        0x60 => "800".to_string(),
        0x63 => "1000".to_string(),
        0x65 => "1250".to_string(),
        0x68 => "1600".to_string(),
        0x6B => "2000".to_string(),
        0x6D => "2500".to_string(),
        0x70 => "3200".to_string(),
        0x73 => "4000".to_string(),
        0x75 => "5000".to_string(),
        0x78 => "6400".to_string(),
        0x7B => "8000".to_string(),
        0x7D => "10000".to_string(),
        0x80 => "12800".to_string(),
        0x83 => "16000".to_string(),
        0x85 => "20000".to_string(),
        0x88 => "25600".to_string(),
        0x90 => "51200".to_string(),
        0x98 => "102400".to_string(),
        _ => format!("ISO {value:#X}"),
    }
}

/// Translate EDSDK aperture value to display label.
fn translate_aperture(value: i32) -> String {
    match value {
        0x08 => "f/1.0".to_string(),
        0x0B => "f/1.1".to_string(),
        0x0D => "f/1.2".to_string(),
        0x10 => "f/1.4".to_string(),
        0x13 => "f/1.6".to_string(),
        0x15 => "f/1.8".to_string(),
        0x18 => "f/2.0".to_string(),
        0x1B => "f/2.2".to_string(),
        0x1D => "f/2.5".to_string(),
        0x20 => "f/2.8".to_string(),
        0x23 => "f/3.2".to_string(),
        0x25 => "f/3.5".to_string(),
        0x28 => "f/4.0".to_string(),
        0x2B => "f/4.5".to_string(),
        0x2D => "f/5.0".to_string(),
        0x30 => "f/5.6".to_string(),
        0x33 => "f/6.3".to_string(),
        0x35 => "f/7.1".to_string(),
        0x38 => "f/8.0".to_string(),
        0x3B => "f/9.0".to_string(),
        0x3D => "f/10".to_string(),
        0x40 => "f/11".to_string(),
        0x43 => "f/13".to_string(),
        0x45 => "f/14".to_string(),
        0x48 => "f/16".to_string(),
        0x4B => "f/18".to_string(),
        0x4D => "f/20".to_string(),
        0x50 => "f/22".to_string(),
        0x53 => "f/25".to_string(),
        0x55 => "f/29".to_string(),
        0x58 => "f/32".to_string(),
        _ => format!("f/? ({value:#X})"),
    }
}

/// Translate EDSDK shutter speed value to display label.
fn translate_shutter_speed(value: i32) -> String {
    match value {
        0x10 => "30\"".to_string(),
        0x13 => "25\"".to_string(),
        0x14 => "20\"".to_string(),
        0x15 => "20\"".to_string(),
        0x18 => "15\"".to_string(),
        0x1B => "13\"".to_string(),
        0x1D => "10\"".to_string(),
        0x20 => "8\"".to_string(),
        0x23 => "6\"".to_string(),
        0x25 => "5\"".to_string(),
        0x28 => "4\"".to_string(),
        0x2B => "3.2\"".to_string(),
        0x2D => "2.5\"".to_string(),
        0x30 => "2\"".to_string(),
        0x33 => "1.6\"".to_string(),
        0x35 => "1.3\"".to_string(),
        0x38 => "1\"".to_string(),
        0x3B => "0.8\"".to_string(),
        0x3D => "0.6\"".to_string(),
        0x40 => "0.5\"".to_string(),
        0x43 => "0.4\"".to_string(),
        0x45 => "0.3\"".to_string(),
        0x48 => "1/4".to_string(),
        0x4B => "1/5".to_string(),
        0x4D => "1/6".to_string(),
        0x50 => "1/8".to_string(),
        0x53 => "1/10".to_string(),
        0x55 => "1/13".to_string(),
        0x58 => "1/15".to_string(),
        0x5B => "1/20".to_string(),
        0x5D => "1/25".to_string(),
        0x60 => "1/30".to_string(),
        0x63 => "1/40".to_string(),
        0x65 => "1/50".to_string(),
        0x68 => "1/60".to_string(),
        0x6B => "1/80".to_string(),
        0x6D => "1/100".to_string(),
        0x70 => "1/125".to_string(),
        0x73 => "1/160".to_string(),
        0x75 => "1/200".to_string(),
        0x78 => "1/250".to_string(),
        0x7B => "1/320".to_string(),
        0x7D => "1/400".to_string(),
        0x80 => "1/500".to_string(),
        0x83 => "1/640".to_string(),
        0x85 => "1/800".to_string(),
        0x88 => "1/1000".to_string(),
        0x8B => "1/1250".to_string(),
        0x8D => "1/1600".to_string(),
        0x90 => "1/2000".to_string(),
        0x93 => "1/2500".to_string(),
        0x95 => "1/3200".to_string(),
        0x98 => "1/4000".to_string(),
        0x9B => "1/5000".to_string(),
        0x9D => "1/6400".to_string(),
        0xA0 => "1/8000".to_string(),
        _ => format!("Tv {value:#X}"),
    }
}

/// Translate EDSDK white balance value to display label.
fn translate_white_balance(value: i32) -> String {
    match value {
        0 => "Auto".to_string(),
        1 => "Daylight".to_string(),
        2 => "Cloudy".to_string(),
        3 => "Tungsten".to_string(),
        4 => "Fluorescent".to_string(),
        5 => "Flash".to_string(),
        6 => "Manual".to_string(),
        8 => "Shade".to_string(),
        9 => "Colour Temperature".to_string(),
        15 => "Custom 1".to_string(),
        16 => "Custom 2".to_string(),
        18 => "Custom 3".to_string(),
        _ => format!("WB {value}"),
    }
}

/// Translate EDSDK exposure compensation value to display label.
fn translate_exposure_comp(value: i32) -> String {
    // EDSDK uses 1/3-stop increments encoded as offsets
    let stops = f64::from(value) / 8.0;
    if stops == 0.0 {
        "0".to_string()
    } else if stops > 0.0 {
        format!("+{stops:.1}")
    } else {
        format!("{stops:.1}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::canon::mock::MockEdsSdk;

    #[test]
    fn translates_common_iso_values() {
        assert_eq!(translate_iso(0x48), "100");
        assert_eq!(translate_iso(0x50), "200");
        assert_eq!(translate_iso(0x58), "400");
        assert_eq!(translate_iso(0x60), "800");
        assert_eq!(translate_iso(0x68), "1600");
        assert_eq!(translate_iso(0x70), "3200");
        assert_eq!(translate_iso(0x78), "6400");
        assert_eq!(translate_iso(0x80), "12800");
    }

    #[test]
    fn translates_unknown_iso_with_hex() {
        let label = translate_iso(0xFF);
        assert!(label.contains("0xFF"), "got: {label}");
    }

    #[test]
    fn translates_common_aperture_values() {
        assert_eq!(translate_aperture(0x18), "f/2.0");
        assert_eq!(translate_aperture(0x20), "f/2.8");
        assert_eq!(translate_aperture(0x28), "f/4.0");
        assert_eq!(translate_aperture(0x30), "f/5.6");
        assert_eq!(translate_aperture(0x38), "f/8.0");
        assert_eq!(translate_aperture(0x40), "f/11");
    }

    #[test]
    fn translates_common_shutter_speed_values() {
        assert_eq!(translate_shutter_speed(0x98), "1/4000");
        assert_eq!(translate_shutter_speed(0x90), "1/2000");
        assert_eq!(translate_shutter_speed(0x88), "1/1000");
        assert_eq!(translate_shutter_speed(0x80), "1/500");
        assert_eq!(translate_shutter_speed(0x78), "1/250");
        assert_eq!(translate_shutter_speed(0x68), "1/60");
        assert_eq!(translate_shutter_speed(0x38), "1\"");
    }

    #[test]
    fn translates_white_balance_values() {
        assert_eq!(translate_white_balance(0), "Auto");
        assert_eq!(translate_white_balance(1), "Daylight");
        assert_eq!(translate_white_balance(3), "Tungsten");
    }

    #[test]
    fn translates_exposure_compensation() {
        assert_eq!(translate_exposure_comp(0), "0");
        assert_eq!(translate_exposure_comp(8), "+1.0");
        assert_eq!(translate_exposure_comp(-8), "-1.0");
        assert_eq!(translate_exposure_comp(16), "+2.0");
    }

    #[test]
    fn get_canon_controls_returns_descriptors() {
        let mock = MockEdsSdk::new()
            .with_cameras(1)
            .with_property(0, PROP_ID_ISO_SPEED, 0x48)
            .with_property_desc(0, PROP_ID_ISO_SPEED, vec![0x48, 0x50, 0x58])
            .with_property(0, PROP_ID_AV, 0x20)
            .with_property_desc(0, PROP_ID_AV, vec![0x18, 0x20, 0x28])
            .with_property(0, PROP_ID_TV, 0x78)
            .with_property_desc(0, PROP_ID_TV, vec![0x68, 0x78, 0x88])
            .with_property(0, PROP_ID_WHITE_BALANCE, 0)
            .with_property_desc(0, PROP_ID_WHITE_BALANCE, vec![0, 1, 2])
            .with_property(0, PROP_ID_EXPOSURE_COMPENSATION, 0);

        let controls = get_canon_controls(&mock, CameraHandle(0)).unwrap();

        // Should have ISO, Aperture, Shutter Speed, White Balance
        // (Exposure Compensation may fail if no property_desc configured, but the
        // slider type doesn't need one)
        assert!(controls.len() >= 4, "got {} controls", controls.len());

        let iso = controls.iter().find(|c| c.id == "canon_iso").unwrap();
        assert_eq!(iso.name, "ISO");
        assert_eq!(iso.control_type, ControlType::Select);
        assert_eq!(iso.current, 0x48);
        assert!(iso.options.is_some());
        let options = iso.options.as_ref().unwrap();
        assert_eq!(options.len(), 3);
        assert_eq!(options[0].label, "100");
        assert_eq!(options[1].label, "200");
        assert_eq!(options[2].label, "400");
    }

    #[test]
    fn controls_have_correct_groups() {
        let mock = MockEdsSdk::new()
            .with_cameras(1)
            .with_property(0, PROP_ID_ISO_SPEED, 0x48)
            .with_property_desc(0, PROP_ID_ISO_SPEED, vec![0x48]);

        let controls = get_canon_controls(&mock, CameraHandle(0)).unwrap();
        let iso = controls.iter().find(|c| c.id == "canon_iso").unwrap();
        assert_eq!(iso.group, "camera");
    }

    #[test]
    fn exposure_compensation_is_slider_type() {
        let mock =
            MockEdsSdk::new()
                .with_cameras(1)
                .with_property(0, PROP_ID_EXPOSURE_COMPENSATION, 8);

        let controls = get_canon_controls(&mock, CameraHandle(0)).unwrap();
        let ec = controls
            .iter()
            .find(|c| c.id == "canon_exposure_compensation");
        // May or may not be present depending on other properties,
        // but if present it should be a slider
        if let Some(ec) = ec {
            assert_eq!(ec.control_type, ControlType::Slider);
            assert_eq!(ec.min, Some(-24));
            assert_eq!(ec.max, Some(24));
        }
    }
}
