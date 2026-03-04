//! IClassFactory implementation that creates `VCamMediaSource` or `VCamActivate`
//! instances depending on the requested interface.

use windows::Win32::Foundation::CLASS_E_NOAGGREGATION;
use windows::Win32::Media::MediaFoundation::IMFActivate;
use windows::Win32::System::Com::{IClassFactory, IClassFactory_Impl};
use windows_core::{implement, IUnknown, Interface, Ref, BOOL, GUID};

use crate::activate::VCamActivate;
use crate::media_source::VCamMediaSource;

/// Class factory for the virtual camera media source.
#[implement(IClassFactory)]
pub(crate) struct VCamClassFactory;

impl IClassFactory_Impl for VCamClassFactory_Impl {
    fn CreateInstance(
        &self,
        punkouter: Ref<IUnknown>,
        riid: *const GUID,
        ppvobject: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        // SAFETY: ppvobject is provided by the COM runtime.
        unsafe { *ppvobject = std::ptr::null_mut() };

        // SAFETY: riid is provided by COM runtime.
        let riid = unsafe { &*riid };
        crate::trace::trace_create_instance(riid);

        // Aggregation not supported.
        if punkouter.is_some() {
            crate::trace::trace("CreateInstance -> CLASS_E_NOAGGREGATION");
            return Err(CLASS_E_NOAGGREGATION.into());
        }

        // FrameServer requests IMFActivate — create the activation wrapper.
        // Our in-process tests request IUnknown/IMFMediaSource — create directly.
        let unknown: IUnknown = if *riid == IMFActivate::IID {
            let activate = VCamActivate::new()?;
            activate.into()
        } else {
            let source = VCamMediaSource::new()?;
            source.into()
        };

        let hr = unsafe { unknown.query(riid, ppvobject) };
        crate::trace::trace_method_result("CreateInstance", hr.0);
        hr.ok()
    }

    fn LockServer(&self, _flock: BOOL) -> windows_core::Result<()> {
        // Not implementing server locking for session-lifetime usage.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Media::MediaFoundation::IMFMediaSource;

    #[test]
    fn factory_creates_media_source() {
        let factory = VCamClassFactory;
        let factory_iface: IClassFactory = factory.into();

        // The windows 0.62 high-level CreateInstance is generic and type-inferred.
        let result: windows_core::Result<IMFMediaSource> =
            unsafe { factory_iface.CreateInstance(None) };

        assert!(result.is_ok());
    }
}
