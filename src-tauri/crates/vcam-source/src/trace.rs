//! File-based tracing for debugging FrameServer interactions.
//!
//! Writes to `%TEMP%\vcam_source_trace.log` in append mode. Every entry is
//! timestamped so we can reconstruct the exact call sequence that FrameServer
//! makes when loading our DLL out-of-process.

use std::fmt::Write as _;
use std::fs::OpenOptions;
use std::io::Write;

use windows_core::GUID;

/// Format a GUID as `{XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX}`.
fn fmt_guid(g: &GUID) -> String {
    let mut s = String::with_capacity(38);
    let _ = write!(
        s,
        "{{{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
        g.data1,
        g.data2,
        g.data3,
        g.data4[0],
        g.data4[1],
        g.data4[2],
        g.data4[3],
        g.data4[4],
        g.data4[5],
        g.data4[6],
        g.data4[7],
    );
    s
}

/// Append a line to the trace log. Silently ignores I/O errors — tracing must
/// never break the DLL.
pub(crate) fn trace(msg: &str) {
    let path = std::env::temp_dir().join("vcam_source_trace.log");
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
        // Simple timestamp: milliseconds since UNIX epoch.
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let _ = writeln!(f, "[{ts}] {msg}");
    }
}

/// Log a DllGetClassObject call.
pub(crate) fn trace_dll_get_class_object(clsid: &GUID, iid: &GUID) {
    trace(&format!(
        "DllGetClassObject clsid={} iid={}",
        fmt_guid(clsid),
        fmt_guid(iid),
    ));
}

/// Log a CreateInstance call.
pub(crate) fn trace_create_instance(iid: &GUID) {
    trace(&format!("CreateInstance iid={}", fmt_guid(iid)));
}

/// Log an interface method call.
pub(crate) fn trace_method(method: &str) {
    trace(&format!("{method} called"));
}

/// Log a GetService call.
pub(crate) fn trace_get_service(guidservice: &GUID, riid: &GUID, result: &str) {
    trace(&format!(
        "GetService guidservice={} riid={} -> {result}",
        fmt_guid(guidservice),
        fmt_guid(riid),
    ));
}

/// Log a method with a result.
pub(crate) fn trace_method_result(method: &str, hr: i32) {
    trace(&format!("{method} -> {hr:#010x}"));
}

/// Log an attribute Set call with a string value.
pub(crate) fn trace_set_string(key: &GUID, value: &str) {
    trace(&format!(
        "IMFAttributes::SetString key={} value={value:?}",
        fmt_guid(key),
    ));
}

/// Log an attribute Set call with a GUID value.
pub(crate) fn trace_set_guid(key: &GUID, value: &GUID) {
    trace(&format!(
        "IMFAttributes::SetGUID key={} value={}",
        fmt_guid(key),
        fmt_guid(value),
    ));
}

/// Log an attribute Set call with a u32 value.
pub(crate) fn trace_set_uint32(key: &GUID, value: u32) {
    trace(&format!(
        "IMFAttributes::SetUINT32 key={} value={value:#010x}",
        fmt_guid(key),
    ));
}

/// Log an attribute Set call with a u64 value.
pub(crate) fn trace_set_uint64(key: &GUID, value: u64) {
    trace(&format!(
        "IMFAttributes::SetUINT64 key={} value={value:#018x}",
        fmt_guid(key),
    ));
}
