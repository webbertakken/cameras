fn main() {
    configure_edsdk();
    tauri_build::build()
}

/// Configure EDSDK linking when the `canon` feature is enabled on Windows.
///
/// Locates the EDSDK library and DLLs from `.proprietary/Canon/...` and emits
/// the appropriate `cargo:rustc-link-search` and `cargo:rustc-link-lib` directives.
/// Also copies `EDSDK.dll` and `EdsImage.dll` to the target directory for runtime use.
///
/// If the DLLs are not found, prints a warning but does not fail the build —
/// this allows mock-only development without the proprietary SDK.
fn configure_edsdk() {
    // Only run when canon feature is enabled on Windows
    if std::env::var("CARGO_FEATURE_CANON").is_err() {
        return;
    }
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() != "windows" {
        return;
    }

    let manifest_dir =
        std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let edsdk_base = manifest_dir
        .join("..")
        .join(".proprietary")
        .join("canon")
        .join("windows");
    let lib_dir = edsdk_base.join("Library");
    let dll_dir = edsdk_base.join("Dll");

    // Check that the library file exists
    let lib_file = lib_dir.join("EDSDK.lib");
    if !lib_file.exists() {
        println!(
            "cargo:warning=Canon EDSDK not found at {}. \
             Build will succeed but linking with --features canon will fail. \
             Place the EDSDK files in .proprietary/canon/windows/",
            lib_file.display()
        );
        return;
    }

    // Emit link search path and library name
    println!(
        "cargo:rustc-link-search=native={}",
        lib_dir
            .canonicalize()
            .expect("canonicalize lib_dir")
            .display()
    );
    println!("cargo:rustc-link-lib=dylib=EDSDK");

    // Copy DLLs to the target directory for runtime availability
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    // OUT_DIR is something like target/debug/build/<crate>/out — walk up to target/debug/
    let target_dir = out_dir
        .ancestors()
        .nth(3)
        .expect("could not find target dir from OUT_DIR");

    for dll_name in &["EDSDK.dll", "EdsImage.dll"] {
        let src = dll_dir.join(dll_name);
        let dst = target_dir.join(dll_name);
        if src.exists() {
            if let Err(e) = std::fs::copy(&src, &dst) {
                println!(
                    "cargo:warning=Failed to copy {} to {}: {}",
                    src.display(),
                    dst.display(),
                    e
                );
            }
        } else {
            println!("cargo:warning=Canon EDSDK DLL not found: {}", src.display());
        }
    }

    // Re-run if the EDSDK files change
    println!("cargo:rerun-if-changed={}", lib_dir.display());
    println!("cargo:rerun-if-changed={}", dll_dir.display());
}
