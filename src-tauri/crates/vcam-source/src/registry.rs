//! Per-user HKCU COM registration helpers.
//!
//! Registers the media source DLL so Windows FrameServer can instantiate it
//! via our CLSID. Uses `HKCU\Software\Classes\CLSID\{...}\InProcServer32`
//! to avoid requiring admin elevation.

#[cfg(windows)]
mod platform {
    use windows::Win32::Foundation::ERROR_SUCCESS;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
        KEY_WRITE, REG_SZ,
    };
    use windows_core::PCWSTR;

    use crate::com_exports::vcam_clsid;

    /// Format the CLSID as a registry-style GUID string: `{XXXXXXXX-XXXX-...}`.
    pub fn clsid_string() -> String {
        let guid = vcam_clsid();
        format!(
            "{{{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
            guid.data1,
            guid.data2,
            guid.data3,
            guid.data4[0],
            guid.data4[1],
            guid.data4[2],
            guid.data4[3],
            guid.data4[4],
            guid.data4[5],
            guid.data4[6],
            guid.data4[7],
        )
    }

    /// The registry path under HKCU for our CLSID's InProcServer32.
    pub fn registry_key_path() -> String {
        format!(r"Software\Classes\CLSID\{}\InProcServer32", clsid_string())
    }

    /// The parent registry path for the CLSID itself.
    pub fn clsid_key_path() -> String {
        format!(r"Software\Classes\CLSID\{}", clsid_string())
    }

    /// Register the COM media source DLL at the given path.
    ///
    /// Creates `HKCU\Software\Classes\CLSID\{CLSID}\InProcServer32` with:
    /// - Default value: `dll_path`
    /// - `ThreadingModel`: `"Both"`
    pub fn register_com_server(dll_path: &str) -> Result<(), String> {
        let key_path = registry_key_path();
        let wide_key = to_wide(&key_path);

        let mut hkey = HKEY::default();
        let result = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(wide_key.as_ptr()),
                Some(0),
                None,
                windows::Win32::System::Registry::REG_OPTION_NON_VOLATILE,
                KEY_WRITE,
                None,
                &mut hkey,
                None,
            )
        };

        if result != ERROR_SUCCESS {
            return Err(format!(
                "Failed to create registry key: error code {}",
                result.0
            ));
        }

        // Set default value to the DLL path.
        let wide_path = to_wide(dll_path);
        let path_bytes = wide_to_bytes(&wide_path);

        let result = unsafe {
            RegSetValueExW(
                hkey,
                None, // Default value
                Some(0),
                REG_SZ,
                Some(&path_bytes),
            )
        };

        if result != ERROR_SUCCESS {
            let _ = unsafe { RegCloseKey(hkey) };
            return Err(format!("Failed to set DLL path: error code {}", result.0));
        }

        // Set ThreadingModel = "Both".
        let threading_model = to_wide("Both");
        let tm_bytes = wide_to_bytes(&threading_model);
        let wide_name = to_wide("ThreadingModel");

        let result = unsafe {
            RegSetValueExW(
                hkey,
                PCWSTR(wide_name.as_ptr()),
                Some(0),
                REG_SZ,
                Some(&tm_bytes),
            )
        };

        let _ = unsafe { RegCloseKey(hkey) };

        if result != ERROR_SUCCESS {
            return Err(format!(
                "Failed to set ThreadingModel: error code {}",
                result.0
            ));
        }

        Ok(())
    }

    /// Unregister the COM server by removing the CLSID key tree.
    pub fn unregister_com_server() -> Result<(), String> {
        let key_path = clsid_key_path();
        let wide_key = to_wide(&key_path);

        let result = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, PCWSTR(wide_key.as_ptr())) };

        if result != ERROR_SUCCESS {
            return Err(format!(
                "Failed to delete registry key: error code {}",
                result.0
            ));
        }

        Ok(())
    }

    /// Encode a Rust `&str` as a null-terminated UTF-16 wide string.
    fn to_wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// Convert a null-terminated UTF-16 slice to a byte slice for registry APIs.
    fn wide_to_bytes(wide: &[u16]) -> Vec<u8> {
        wide.iter().flat_map(|w| w.to_le_bytes()).collect()
    }
}

#[cfg(windows)]
pub use platform::{
    clsid_key_path, clsid_string, register_com_server, registry_key_path, unregister_com_server,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clsid_string_format() {
        let s = clsid_string();
        // Should be in {XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX} format.
        assert!(s.starts_with('{'));
        assert!(s.ends_with('}'));
        assert_eq!(s.len(), 38); // 36 hex chars + 2 braces
        assert_eq!(s.matches('-').count(), 4);
    }

    #[test]
    fn registry_key_path_contains_clsid() {
        let path = registry_key_path();
        assert!(path.starts_with(r"Software\Classes\CLSID\{"));
        assert!(path.ends_with(r"\InProcServer32"));
        assert!(path.contains(&clsid_string()));
    }

    #[test]
    fn clsid_key_path_is_parent() {
        let parent = clsid_key_path();
        let child = registry_key_path();
        assert!(child.starts_with(&parent));
    }
}
