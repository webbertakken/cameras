fn main() {
    configure_edsdk();
    copy_vcam_source_dll();
    tauri_build::build()
}

/// Ensure `vcam_source.dll` is present in the target directory.
///
/// Since `vcam-source` is a workspace member, its DLL lands in the shared
/// `target/{profile}/` directory alongside `cameras.exe`. This function
/// verifies the DLL exists and emits a `rerun-if-changed` directive so
/// Cargo rebuilds when the DLL is updated.
///
/// Build `vcam-source` first: `cargo build -p vcam-source`
fn copy_vcam_source_dll() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "windows" {
        return;
    }

    let target_dir = resolve_target_dir();
    let dll_path = target_dir.join("vcam_source.dll");

    if !dll_path.exists() {
        println!(
            "cargo:warning=vcam_source.dll not found at {}. \
             Build it with `cargo build -p vcam-source`.",
            dll_path.display()
        );
    }

    println!("cargo:rerun-if-changed={}", dll_path.display());
}

/// Configure EDSDK linking when the `canon` feature is enabled.
///
/// Locates the platform-specific EDSDK libraries from `.proprietary/canon/`
/// and emits the appropriate linker directives. Also copies runtime libraries
/// to the target directory.
///
/// If the SDK is not found, prints a warning but does not fail the build —
/// this allows mock-only development without the proprietary SDK.
fn configure_edsdk() {
    if std::env::var("CARGO_FEATURE_CANON").is_err() {
        return;
    }

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();

    let manifest_dir =
        std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let canon_base = manifest_dir.join("..").join(".proprietary").join("canon");

    match target_os.as_str() {
        "windows" => configure_edsdk_windows(&canon_base),
        "linux" => configure_edsdk_linux(&canon_base, &target_arch),
        "macos" => configure_edsdk_macos(&canon_base),
        _ => {}
    }
}

/// Windows: link EDSDK.lib, copy EDSDK.dll + EdsImage.dll to target dir.
fn configure_edsdk_windows(canon_base: &std::path::Path) {
    let sdk_dir = canon_base.join("windows");
    let lib_dir = sdk_dir.join("Library");
    let dll_dir = sdk_dir.join("Dll");

    let lib_file = lib_dir.join("EDSDK.lib");
    if !lib_file.exists() {
        println!(
            "cargo:warning=Canon EDSDK not found at {}. \
             Place the EDSDK files in .proprietary/canon/windows/",
            lib_file.display()
        );
        return;
    }

    println!(
        "cargo:rustc-link-search=native={}",
        lib_dir
            .canonicalize()
            .expect("canonicalize lib_dir")
            .display()
    );
    println!("cargo:rustc-link-lib=dylib=EDSDK");

    // Copy DLLs to the target directory for runtime availability
    let target_dir = resolve_target_dir();
    for dll_name in &["EDSDK.dll", "EdsImage.dll"] {
        copy_sdk_file(&dll_dir.join(dll_name), &target_dir.join(dll_name));
    }

    println!("cargo:rerun-if-changed={}", lib_dir.display());
    println!("cargo:rerun-if-changed={}", dll_dir.display());
}

/// Linux: link libEDSDK.so, copy to target dir.
fn configure_edsdk_linux(canon_base: &std::path::Path, target_arch: &str) {
    let arch_dir = match target_arch {
        "x86_64" => "linux-x64",
        "aarch64" => "linux-arm64",
        other => {
            println!("cargo:warning=Canon EDSDK: unsupported Linux arch: {other}");
            return;
        }
    };

    let sdk_dir = canon_base.join(arch_dir);
    let so_file = sdk_dir.join("libEDSDK.so");
    if !so_file.exists() {
        println!(
            "cargo:warning=Canon EDSDK not found at {}. \
             Place libEDSDK.so in .proprietary/canon/{arch_dir}/",
            so_file.display()
        );
        return;
    }

    println!(
        "cargo:rustc-link-search=native={}",
        sdk_dir
            .canonicalize()
            .expect("canonicalize sdk_dir")
            .display()
    );
    println!("cargo:rustc-link-lib=dylib=EDSDK");

    // Copy .so to target dir for runtime availability
    let target_dir = resolve_target_dir();
    copy_sdk_file(&so_file, &target_dir.join("libEDSDK.so"));

    println!("cargo:rerun-if-changed={}", sdk_dir.display());
}

/// macOS: link EDSDK.framework, copy to target dir.
fn configure_edsdk_macos(canon_base: &std::path::Path) {
    let sdk_dir = canon_base.join("macos");
    let framework = sdk_dir.join("EDSDK.framework");
    if !framework.exists() {
        println!(
            "cargo:warning=Canon EDSDK.framework not found at {}. \
             Place EDSDK.framework in .proprietary/canon/macos/",
            framework.display()
        );
        return;
    }

    println!(
        "cargo:rustc-link-search=framework={}",
        sdk_dir
            .canonicalize()
            .expect("canonicalize sdk_dir")
            .display()
    );
    println!("cargo:rustc-link-lib=framework=EDSDK");

    println!("cargo:rerun-if-changed={}", sdk_dir.display());
}

/// Resolve the target output directory from OUT_DIR.
/// OUT_DIR is something like target/debug/build/<crate>/out — walk up to target/debug/.
fn resolve_target_dir() -> std::path::PathBuf {
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    out_dir
        .ancestors()
        .nth(3)
        .expect("could not find target dir from OUT_DIR")
        .to_path_buf()
}

/// Copy a single SDK file, warning on failure.
fn copy_sdk_file(src: &std::path::Path, dst: &std::path::Path) {
    if src.exists() {
        if let Err(e) = std::fs::copy(src, dst) {
            println!(
                "cargo:warning=Failed to copy {} to {}: {}",
                src.display(),
                dst.display(),
                e
            );
        }
    } else {
        println!(
            "cargo:warning=Canon EDSDK file not found: {}",
            src.display()
        );
    }
}
