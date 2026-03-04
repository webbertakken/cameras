//! Standalone diagnostic binary for virtual camera COM interface testing.
//!
//! Steps:
//!  1. Locates the vcam_source.dll.
//!  2. CoCreates the media source object.
//!  3. Tests QueryInterface for every relevant interface.
//!  4. Calls `MFCreateVirtualCamera` + `Start()` and reports the result.
//!
//! The MSIX sparse package must already be registered (done at install time
//! by the NSIS post-install hook) for COM redirection to work.

#![cfg(windows)]

use std::env;
use std::path::PathBuf;

use windows::core::{Interface, GUID, HSTRING};
use windows::Win32::Media::KernelStreaming::{IKsControl, IKsPropertySet, KSCATEGORY_VIDEO_CAMERA};
use windows::Win32::Media::MediaFoundation::{
    IMFActivate, IMFAttributes, IMFGetService, IMFMediaEventGenerator, IMFMediaSource,
    IMFMediaSourceEx, IMFRealTimeClientEx, IMFSampleAllocatorControl, IMFShutdown,
    IMFVirtualCamera, MFCreateVirtualCamera, MFStartup, MFVirtualCameraAccess_CurrentUser,
    MFVirtualCameraLifetime_Session, MFVirtualCameraType_SoftwareCameraSource,
    MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE, MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_CATEGORY,
    MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID, MF_MEDIASOURCE_SERVICE, MF_VERSION,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, IClassFactory, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};
use windows_core::IUnknown;

const VCAM_CLSID: GUID = GUID::from_u128(vcam_shared::VCAM_SOURCE_CLSID);

fn main() {
    let args: Vec<String> = env::args().collect();
    let help = args.iter().any(|a| a == "--help" || a == "-h");

    if help {
        println!("Usage: vcam-test [OPTIONS]");
        println!();
        println!("Options:");
        println!("  --help, -h  Show this help message");
        println!();
        println!("Note: The MSIX sparse package must be registered at install time");
        println!("      for COM redirection to work. See NSIS post-install hook.");
        return;
    }

    println!("=== vcam-test: MF Virtual Camera diagnostic ===\n");

    // Step 1: Locate the DLL.
    let dll_path = match locate_dll() {
        Ok(p) => p,
        Err(e) => {
            println!("[FAIL] Could not locate vcam_source.dll: {e}");
            println!("       Build it with: cargo build -p vcam-source");
            println!("       Then place it next to vcam-test.exe");
            std::process::exit(1);
        }
    };
    println!("[INFO] DLL path: {}", dll_path.display());

    println!();

    // Step 2: CoInitialize.
    let co_hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    if co_hr.is_ok() {
        println!("[PASS] CoInitializeEx(MULTITHREADED)");
    } else if co_hr.0 == 0x80010106u32 as i32 {
        // RPC_E_CHANGED_MODE — apartment already exists, that's fine.
        println!("[PASS] CoInitializeEx — apartment already initialised (RPC_E_CHANGED_MODE, OK)");
    } else {
        println!("[FAIL] CoInitializeEx: {:#010x}", co_hr.0);
        std::process::exit(1);
    }

    // MFStartup is required before using any MF APIs.
    match unsafe { MFStartup(MF_VERSION, 0) } {
        Ok(()) => println!("[PASS] MFStartup"),
        Err(e) => println!("[WARN] MFStartup: {e} (may be benign if already started)"),
    }

    println!();

    // Step 3: CoCreateInstance our CLSID.
    println!("--- CoCreateInstance ---");
    let obj: IUnknown = match unsafe { CoCreateInstance(&VCAM_CLSID, None, CLSCTX_INPROC_SERVER) } {
        Ok(o) => {
            println!("[PASS] CoCreateInstance -> IUnknown");
            o
        }
        Err(e) => {
            println!(
                "[FAIL] CoCreateInstance failed: {:#010x} ({})",
                e.code().0,
                e
            );
            println!("       Hint: is the MSIX sparse package registered?");
            std::process::exit(1);
        }
    };

    println!();

    // Step 4: Test every interface via QueryInterface.
    println!("--- QueryInterface tests ---");

    test_qi::<IClassFactory>(&obj, "IClassFactory (expected to fail)");
    test_qi::<IMFMediaSource>(&obj, "IMFMediaSource");
    test_qi::<IMFMediaEventGenerator>(&obj, "IMFMediaEventGenerator");
    test_qi::<IKsControl>(&obj, "IKsControl");
    test_qi::<IKsPropertySet>(&obj, "IKsPropertySet");
    test_qi::<IMFMediaSourceEx>(&obj, "IMFMediaSourceEx");
    test_qi::<IMFGetService>(&obj, "IMFGetService");
    test_qi::<IMFRealTimeClientEx>(&obj, "IMFRealTimeClientEx");
    test_qi::<IMFShutdown>(&obj, "IMFShutdown");
    test_qi::<IMFSampleAllocatorControl>(&obj, "IMFSampleAllocatorControl");

    println!();

    // Step 5: Verify GetSourceAttributes returns the required GUIDs.
    println!("--- GetSourceAttributes ---");
    test_source_attributes(&obj);

    println!();

    // Step 6: Verify GetService(MF_MEDIASOURCE_SERVICE) returns IKsControl.
    println!("--- GetService ---");
    test_get_service(&obj);

    println!();

    // Step 7: Test IMFActivate creation via CoCreateInstance.
    println!("--- IMFActivate via CoCreateInstance ---");
    test_activate_creation();

    println!();

    // Step 8: MFCreateVirtualCamera + Start().
    println!("--- MFCreateVirtualCamera + Start ---");
    test_mf_virtual_camera();

    println!("\n=== Diagnostic complete ===");
}

// ---------------------------------------------------------------------------
// DLL location
// ---------------------------------------------------------------------------

/// Locate `vcam_source.dll` — first next to this binary, then in target/debug.
fn locate_dll() -> Result<PathBuf, String> {
    let exe = env::current_exe().map_err(|e| format!("cannot get exe path: {e}"))?;
    let exe_dir = exe.parent().ok_or("exe has no parent dir")?;

    // Check next to the binary first.
    let beside = exe_dir.join("vcam_source.dll");
    if beside.exists() {
        return Ok(beside);
    }

    // Walk up to find the workspace target/debug directory.
    let mut cur = exe_dir.to_path_buf();
    for _ in 0..5 {
        let candidate = cur.join("debug").join("vcam_source.dll");
        if candidate.exists() {
            return Ok(candidate);
        }
        let candidate2 = cur.join("vcam_source.dll");
        if candidate2.exists() {
            return Ok(candidate2);
        }
        if !cur.pop() {
            break;
        }
    }

    Err(format!(
        "vcam_source.dll not found near {} or in parent directories",
        exe_dir.display()
    ))
}

// ---------------------------------------------------------------------------
// Interface and attribute tests
// ---------------------------------------------------------------------------

/// Verify `GetSourceAttributes` returns the two required FrameServer GUIDs.
fn test_source_attributes(obj: &IUnknown) {
    let source: IMFMediaSourceEx = match obj.cast() {
        Ok(s) => s,
        Err(e) => {
            println!(
                "[FAIL] Cast to IMFMediaSourceEx: {:#010x} ({})",
                e.code().0,
                e
            );
            return;
        }
    };

    let attrs: IMFAttributes = match unsafe { source.GetSourceAttributes() } {
        Ok(a) => {
            println!("[PASS] GetSourceAttributes returned attribute store");
            a
        }
        Err(e) => {
            println!("[FAIL] GetSourceAttributes: {:#010x} ({})", e.code().0, e);
            return;
        }
    };

    // Check MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE == MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID
    match unsafe { attrs.GetGUID(&MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE) } {
        Ok(guid) if guid == MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID => {
            println!("[PASS] SOURCE_TYPE = VIDCAP_GUID");
        }
        Ok(guid) => {
            println!("[FAIL] SOURCE_TYPE has wrong GUID: {guid:?}");
        }
        Err(e) => {
            println!("[FAIL] SOURCE_TYPE missing: {:#010x} ({})", e.code().0, e);
        }
    }

    // Check MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_CATEGORY == KSCATEGORY_VIDEO_CAMERA
    match unsafe { attrs.GetGUID(&MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_CATEGORY) } {
        Ok(guid) if guid == KSCATEGORY_VIDEO_CAMERA => {
            println!("[PASS] VIDCAP_CATEGORY = KSCATEGORY_VIDEO_CAMERA");
        }
        Ok(guid) => {
            println!("[FAIL] VIDCAP_CATEGORY has wrong GUID: {guid:?}");
        }
        Err(e) => {
            println!(
                "[FAIL] VIDCAP_CATEGORY missing: {:#010x} ({})",
                e.code().0,
                e
            );
        }
    }
}

/// Verify `GetService(MF_MEDIASOURCE_SERVICE)` returns a valid `IKsControl`.
fn test_get_service(obj: &IUnknown) {
    let svc: IMFGetService = match obj.cast() {
        Ok(s) => s,
        Err(e) => {
            println!("[FAIL] Cast to IMFGetService: {:#010x} ({})", e.code().0, e);
            return;
        }
    };

    match unsafe { svc.GetService::<IKsControl>(&MF_MEDIASOURCE_SERVICE) } {
        Ok(_iks) => {
            println!("[PASS] GetService(MF_MEDIASOURCE_SERVICE) -> IKsControl");
        }
        Err(e) => {
            println!(
                "[FAIL] GetService(MF_MEDIASOURCE_SERVICE): {:#010x} ({})",
                e.code().0,
                e
            );
        }
    }
}

/// Try to QueryInterface for interface T, print pass/fail.
fn test_qi<T: Interface>(obj: &IUnknown, label: &str) {
    match obj.cast::<T>() {
        Ok(_) => println!("[PASS] QI {label}"),
        Err(e) => println!("[FAIL] QI {label}: {:#010x} ({})", e.code().0, e),
    }
}

/// Test that CoCreateInstance with IMFActivate::IID works.
fn test_activate_creation() {
    // Use CreateInstance directly via the class factory to test IMFActivate path.
    let activate: windows_core::Result<IMFActivate> =
        unsafe { CoCreateInstance(&VCAM_CLSID, None, CLSCTX_INPROC_SERVER) };
    match activate {
        Ok(act) => {
            println!("[PASS] CoCreateInstance -> IMFActivate");

            // Test ActivateObject — should return an IMFMediaSource.
            match unsafe { act.ActivateObject::<IMFMediaSource>() } {
                Ok(_source) => println!("[PASS] ActivateObject -> IMFMediaSource"),
                Err(e) => println!("[FAIL] ActivateObject: {:#010x} ({})", e.code().0, e),
            }

            // Test ShutdownObject.
            match unsafe { act.ShutdownObject() } {
                Ok(()) => println!("[PASS] ShutdownObject"),
                Err(e) => println!("[FAIL] ShutdownObject: {:#010x} ({})", e.code().0, e),
            }
        }
        Err(e) => {
            println!(
                "[FAIL] CoCreateInstance -> IMFActivate: {:#010x} ({})",
                e.code().0,
                e
            );
        }
    }
}

/// Calls `MFCreateVirtualCamera` and then `Start()`, reporting each result.
fn test_mf_virtual_camera() {
    let clsid_str = format_clsid_str(VCAM_CLSID);
    let friendly = HSTRING::from("vcam-test diagnostic camera");
    let clsid_h = HSTRING::from(&clsid_str);
    let categories = [KSCATEGORY_VIDEO_CAMERA];

    println!("[INFO] CLSID: {clsid_str}");

    let vcam: IMFVirtualCamera = match unsafe {
        MFCreateVirtualCamera(
            MFVirtualCameraType_SoftwareCameraSource,
            MFVirtualCameraLifetime_Session,
            MFVirtualCameraAccess_CurrentUser,
            &friendly,
            &clsid_h,
            Some(&categories),
        )
    } {
        Ok(v) => {
            println!("[PASS] MFCreateVirtualCamera");
            v
        }
        Err(e) => {
            println!("[FAIL] MFCreateVirtualCamera: {:#010x} ({})", e.code().0, e);
            return;
        }
    };

    // Try Start().
    match unsafe { vcam.Start(None) } {
        Ok(()) => println!("[PASS] IMFVirtualCamera::Start -> S_OK"),
        Err(e) => println!(
            "[FAIL] IMFVirtualCamera::Start: {:#010x} ({})",
            e.code().0,
            e
        ),
    }

    // Clean up.
    let _ = unsafe { vcam.Stop() };
    let _ = unsafe { vcam.Shutdown() };
}

/// Format a GUID as `{XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX}`.
fn format_clsid_str(guid: GUID) -> String {
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
