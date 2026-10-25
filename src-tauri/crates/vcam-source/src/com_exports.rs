//! DLL entry points for COM: `DllGetClassObject` and `DllCanUnloadNow`.

use std::sync::atomic::Ordering;

use windows::core::{Interface, GUID};
use windows::Win32::Foundation::{CLASS_E_CLASSNOTAVAILABLE, E_POINTER, S_FALSE, S_OK};
use windows::Win32::System::Com::IClassFactory;
use windows_core::HRESULT;

use crate::class_factory::VCamClassFactory;
use crate::ACTIVE_OBJECTS;

/// The CLSID as a `windows::core::GUID`.
pub(crate) fn vcam_clsid() -> GUID {
    // {7B2E3A1F-4D5C-4E8B-9A6F-1C2D3E4F5A6B}
    GUID::from_u128(crate::VCAM_SOURCE_CLSID)
}

/// COM entry point: creates the class factory for our CLSID.
///
/// # Safety
/// Called by COM runtime. `ppv` must be a valid out-pointer.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllGetClassObject(
    rclsid: *const GUID,
    riid: *const GUID,
    ppv: *mut *mut core::ffi::c_void,
) -> HRESULT {
    if ppv.is_null() {
        return E_POINTER;
    }
    // SAFETY: caller guarantees ppv is valid.
    unsafe { *ppv = std::ptr::null_mut() };

    // SAFETY: caller guarantees rclsid and riid point to valid GUIDs.
    let rclsid = unsafe { &*rclsid };
    let riid = unsafe { &*riid };

    crate::trace::trace_dll_get_class_object(rclsid, riid);

    if *rclsid != vcam_clsid() {
        crate::trace::trace("DllGetClassObject -> CLASS_E_CLASSNOTAVAILABLE");
        return CLASS_E_CLASSNOTAVAILABLE;
    }

    // Create the factory and use proper QueryInterface to handle IUnknown and
    // any other valid riid, rather than hard-rejecting non-IClassFactory riids.
    let factory: IClassFactory = VCamClassFactory.into();
    // SAFETY: ppv is valid; query() writes the correctly ref-counted pointer.
    let hr = unsafe { factory.query(riid, ppv) };
    crate::trace::trace_method_result("DllGetClassObject", hr.0);
    hr
}

/// COM entry point: returns `S_OK` if the DLL can be unloaded (no live objects).
#[unsafe(no_mangle)]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    if ACTIVE_OBJECTS.load(Ordering::Relaxed) == 0 {
        S_OK
    } else {
        S_FALSE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vcam_clsid_roundtrip() {
        let guid = vcam_clsid();
        assert_eq!(guid, GUID::from_u128(crate::VCAM_SOURCE_CLSID));
    }

    #[test]
    fn dll_can_unload_when_no_objects() {
        let prev = ACTIVE_OBJECTS.load(Ordering::Relaxed);
        ACTIVE_OBJECTS.store(0, Ordering::Relaxed);

        assert_eq!(DllCanUnloadNow(), S_OK);

        ACTIVE_OBJECTS.store(prev, Ordering::Relaxed);
    }

    #[test]
    fn dll_cannot_unload_with_active_objects() {
        let prev = ACTIVE_OBJECTS.load(Ordering::Relaxed);
        ACTIVE_OBJECTS.store(1, Ordering::Relaxed);

        assert_eq!(DllCanUnloadNow(), S_FALSE);

        ACTIVE_OBJECTS.store(prev, Ordering::Relaxed);
    }
}
