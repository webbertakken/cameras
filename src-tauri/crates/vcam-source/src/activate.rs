//! `IMFActivate` implementation for the virtual camera.
//!
//! FrameServer's `IMFVirtualCamera::Start` calls `IClassFactory::CreateInstance`
//! requesting `IMFActivate`. The activate object is a wrapper that can lazily
//! create and destroy the underlying `IMFMediaSource` on demand.
//!
//! `IMFActivate` extends `IMFAttributes`, so we must implement the full
//! 30-method attribute interface. We delegate every attribute call to an
//! inner `IMFAttributes` store created via `MFCreateAttributes`.

use std::sync::Mutex;

use windows::Win32::Media::MediaFoundation::{
    IMFActivate, IMFActivate_Impl, IMFAttributes, IMFAttributes_Impl, IMFMediaSourceEx,
    MFCreateAttributes, MF_ATTRIBUTES_MATCH_TYPE, MF_ATTRIBUTE_TYPE, MF_E_SHUTDOWN,
};
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows_core::{implement, IUnknown, Interface, Ref, BOOL, GUID, PCWSTR, PWSTR};

use crate::media_source::VCamMediaSource;

/// Activation object for the virtual camera media source.
///
/// FrameServer creates an instance of this via `IClassFactory::CreateInstance`
/// with `riid = IMFActivate::IID`. It then calls `ActivateObject` to obtain
/// the `IMFMediaSource`, and `ShutdownObject` to tear it down.
#[implement(IMFActivate)]
pub(crate) struct VCamActivate {
    /// Inner attribute store — all `IMFAttributes` calls are delegated here.
    attributes: IMFAttributes,
    /// Cached media source created by `ActivateObject`.
    source: Mutex<Option<IMFMediaSourceEx>>,
}

impl VCamActivate {
    /// Create a new activation object with an empty attribute store.
    pub(crate) fn new() -> windows_core::Result<Self> {
        let mut attrs: Option<IMFAttributes> = None;
        unsafe { MFCreateAttributes(&mut attrs, 0)? };
        let attributes = attrs.ok_or(windows_core::Error::from(MF_E_SHUTDOWN))?;

        Ok(Self {
            attributes,
            source: Mutex::new(None),
        })
    }
}

impl IMFActivate_Impl for VCamActivate_Impl {
    fn ActivateObject(
        &self,
        riid: *const GUID,
        ppv: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        crate::trace::trace_method("IMFActivate::ActivateObject");

        let mut cached = self.source.lock().unwrap();

        // Reuse existing source if available.
        let source: IMFMediaSourceEx = if let Some(ref s) = *cached {
            s.clone()
        } else {
            // Forward the activate's attribute store to the media source so
            // FrameServer-set attributes (symbolic link, etc.) are visible via
            // GetSourceAttributes.
            let ms = VCamMediaSource::new_with_attributes(self.attributes.clone())?;
            let iface: IMFMediaSourceEx = ms.into();
            *cached = Some(iface.clone());
            iface
        };

        // QI the source for the requested interface.
        let riid = unsafe { &*riid };
        let unknown: IUnknown = source.cast()?;
        let hr = unsafe { unknown.query(riid, ppv) };
        crate::trace::trace_method_result("IMFActivate::ActivateObject", hr.0);
        hr.ok()
    }

    fn ShutdownObject(&self) -> windows_core::Result<()> {
        crate::trace::trace_method("IMFActivate::ShutdownObject");

        let mut cached = self.source.lock().unwrap();
        if let Some(source) = cached.take() {
            // Shutdown is on IMFMediaSource, which IMFMediaSourceEx derefs to.
            unsafe { source.Shutdown()? };
        }
        Ok(())
    }

    fn DetachObject(&self) -> windows_core::Result<()> {
        crate::trace::trace_method("IMFActivate::DetachObject");

        // Clear the cached source without shutting it down.
        let mut cached = self.source.lock().unwrap();
        *cached = None;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// IMFAttributes delegation — all 30 methods forwarded to self.attributes
// ---------------------------------------------------------------------------

impl IMFAttributes_Impl for VCamActivate_Impl {
    fn GetItem(&self, guidkey: *const GUID, pvalue: *mut PROPVARIANT) -> windows_core::Result<()> {
        unsafe { self.attributes.GetItem(guidkey, Some(pvalue)) }
    }

    fn GetItemType(&self, guidkey: *const GUID) -> windows_core::Result<MF_ATTRIBUTE_TYPE> {
        unsafe { self.attributes.GetItemType(guidkey) }
    }

    fn CompareItem(
        &self,
        guidkey: *const GUID,
        value: *const PROPVARIANT,
    ) -> windows_core::Result<BOOL> {
        unsafe { self.attributes.CompareItem(guidkey, value) }
    }

    fn Compare(
        &self,
        ptheirs: Ref<IMFAttributes>,
        matchtype: MF_ATTRIBUTES_MATCH_TYPE,
    ) -> windows_core::Result<BOOL> {
        unsafe { self.attributes.Compare(ptheirs.as_ref(), matchtype) }
    }

    fn GetUINT32(&self, guidkey: *const GUID) -> windows_core::Result<u32> {
        unsafe { self.attributes.GetUINT32(guidkey) }
    }

    fn GetUINT64(&self, guidkey: *const GUID) -> windows_core::Result<u64> {
        unsafe { self.attributes.GetUINT64(guidkey) }
    }

    fn GetDouble(&self, guidkey: *const GUID) -> windows_core::Result<f64> {
        unsafe { self.attributes.GetDouble(guidkey) }
    }

    fn GetGUID(&self, guidkey: *const GUID) -> windows_core::Result<GUID> {
        unsafe { self.attributes.GetGUID(guidkey) }
    }

    fn GetStringLength(&self, guidkey: *const GUID) -> windows_core::Result<u32> {
        unsafe { self.attributes.GetStringLength(guidkey) }
    }

    fn GetString(
        &self,
        guidkey: *const GUID,
        pwszvalue: PWSTR,
        cchbufsize: u32,
        pcchlength: *mut u32,
    ) -> windows_core::Result<()> {
        unsafe {
            let buf = std::slice::from_raw_parts_mut(pwszvalue.0, cchbufsize as usize);
            self.attributes.GetString(guidkey, buf, Some(pcchlength))
        }
    }

    fn GetAllocatedString(
        &self,
        guidkey: *const GUID,
        ppwszvalue: *mut PWSTR,
        pcchlength: *mut u32,
    ) -> windows_core::Result<()> {
        unsafe {
            self.attributes
                .GetAllocatedString(guidkey, ppwszvalue, pcchlength)
        }
    }

    fn GetBlobSize(&self, guidkey: *const GUID) -> windows_core::Result<u32> {
        unsafe { self.attributes.GetBlobSize(guidkey) }
    }

    fn GetBlob(
        &self,
        guidkey: *const GUID,
        pbuf: *mut u8,
        cbbufsize: u32,
        pcbblobsize: *mut u32,
    ) -> windows_core::Result<()> {
        unsafe {
            let buf = std::slice::from_raw_parts_mut(pbuf, cbbufsize as usize);
            self.attributes.GetBlob(guidkey, buf, Some(pcbblobsize))
        }
    }

    fn GetAllocatedBlob(
        &self,
        guidkey: *const GUID,
        ppbuf: *mut *mut u8,
        pcbsize: *mut u32,
    ) -> windows_core::Result<()> {
        unsafe { self.attributes.GetAllocatedBlob(guidkey, ppbuf, pcbsize) }
    }

    fn GetUnknown(
        &self,
        guidkey: *const GUID,
        riid: *const GUID,
        ppv: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        // The consumer-side GetUnknown is generic (`GetUnknown<T>`), but the
        // impl trait receives raw (guidkey, riid, ppv). Use Interface::vtable
        // to access the function pointer safely instead of manual raw pointer
        // dereference.
        unsafe {
            (Interface::vtable(&self.attributes).GetUnknown)(
                Interface::as_raw(&self.attributes),
                guidkey,
                riid,
                ppv,
            )
            .ok()
        }
    }

    fn SetItem(&self, guidkey: *const GUID, value: *const PROPVARIANT) -> windows_core::Result<()> {
        unsafe { self.attributes.SetItem(guidkey, value) }
    }

    fn DeleteItem(&self, guidkey: *const GUID) -> windows_core::Result<()> {
        unsafe { self.attributes.DeleteItem(guidkey) }
    }

    fn DeleteAllItems(&self) -> windows_core::Result<()> {
        unsafe { self.attributes.DeleteAllItems() }
    }

    fn SetUINT32(&self, guidkey: *const GUID, unvalue: u32) -> windows_core::Result<()> {
        // SAFETY: guidkey is provided by the COM caller.
        crate::trace::trace_set_uint32(unsafe { &*guidkey }, unvalue);
        unsafe { self.attributes.SetUINT32(guidkey, unvalue) }
    }

    fn SetUINT64(&self, guidkey: *const GUID, unvalue: u64) -> windows_core::Result<()> {
        // SAFETY: guidkey is provided by the COM caller.
        crate::trace::trace_set_uint64(unsafe { &*guidkey }, unvalue);
        unsafe { self.attributes.SetUINT64(guidkey, unvalue) }
    }

    fn SetDouble(&self, guidkey: *const GUID, fvalue: f64) -> windows_core::Result<()> {
        unsafe { self.attributes.SetDouble(guidkey, fvalue) }
    }

    fn SetGUID(&self, guidkey: *const GUID, guidvalue: *const GUID) -> windows_core::Result<()> {
        // SAFETY: guidkey and guidvalue are provided by the COM caller.
        crate::trace::trace_set_guid(unsafe { &*guidkey }, unsafe { &*guidvalue });
        unsafe { self.attributes.SetGUID(guidkey, guidvalue) }
    }

    fn SetString(&self, guidkey: *const GUID, wszvalue: &PCWSTR) -> windows_core::Result<()> {
        // SAFETY: guidkey and wszvalue are provided by the COM caller.
        let key = unsafe { &*guidkey };
        let value = unsafe { wszvalue.to_string().unwrap_or_default() };
        crate::trace::trace_set_string(key, &value);
        unsafe { self.attributes.SetString(guidkey, *wszvalue) }
    }

    fn SetBlob(
        &self,
        guidkey: *const GUID,
        pbuf: *const u8,
        cbbufsize: u32,
    ) -> windows_core::Result<()> {
        unsafe {
            let buf = std::slice::from_raw_parts(pbuf, cbbufsize as usize);
            self.attributes.SetBlob(guidkey, buf)
        }
    }

    fn SetUnknown(
        &self,
        guidkey: *const GUID,
        punknown: Ref<IUnknown>,
    ) -> windows_core::Result<()> {
        unsafe { self.attributes.SetUnknown(guidkey, punknown.as_ref()) }
    }

    fn LockStore(&self) -> windows_core::Result<()> {
        unsafe { self.attributes.LockStore() }
    }

    fn UnlockStore(&self) -> windows_core::Result<()> {
        unsafe { self.attributes.UnlockStore() }
    }

    fn GetCount(&self) -> windows_core::Result<u32> {
        unsafe { self.attributes.GetCount() }
    }

    fn GetItemByIndex(
        &self,
        unindex: u32,
        pguidkey: *mut GUID,
        pvalue: *mut PROPVARIANT,
    ) -> windows_core::Result<()> {
        unsafe {
            self.attributes
                .GetItemByIndex(unindex, pguidkey, Some(pvalue))
        }
    }

    fn CopyAllItems(&self, pdest: Ref<IMFAttributes>) -> windows_core::Result<()> {
        unsafe { self.attributes.CopyAllItems(pdest.as_ref()) }
    }
}
