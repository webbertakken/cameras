# Virtual camera landscape research

## Executive summary

**Every virtual camera on Windows requires admin privileges at install time.** OBS, Elgato, Snap Camera, ManyCam, and all others register their COM components via their installer (which runs elevated). No product has solved the "zero admin" problem -- they all side-step it by doing the privileged work during installation and then running unprivileged at runtime.

Our `MFCreateVirtualCamera` approach is actually the correct modern approach, but we hit the same wall everyone else hits: the media source DLL must be registered in HKLM (or via a WHQL-certified driver package, or via MSIX COM redirection), and all three paths require elevation at some point.

The good news: our existing NSIS installer already has UAC elevation, so the MSIX sparse package registration at install time (our current plan) aligns perfectly with the industry pattern.

---

## 1. How OBS does it

### Technology

**DirectShow source filter** (`obs-virtualcam-module32.dll` and `obs-virtualcam-module64.dll`).

### Registration

- The OBS **installer** (NSIS) calls `regsvr32` to register both 32-bit and 64-bit DLLs during installation
- Writes to `HKEY_CLASSES_ROOT\CLSID\{GUID}` (maps to HKLM) with `InprocServer32` and threading model
- Also registers with DirectShow's `IFilterMapper2` under "Video Input Device Category"
- A `virtualcam-install.bat` ships in the install directory for manual re-registration
- The auto-updater also re-registers the DLLs

### Admin requirement

**Yes -- at install time.** The installer runs elevated. Once registered, OBS itself runs as a standard user and the virtual camera works without elevation.

### Frame delivery

- Shared memory queue with 3 frame slots (minimised from the original 10-20)
- NV12 format with optional I420 conversion
- OBS process writes to shared memory; the DirectShow filter DLL (loaded by the consumer app) reads from it

### Compatibility limitation

**DirectShow virtual cameras are invisible to Media Foundation apps.** This means OBS's virtual camera does not appear in some modern apps that use MF exclusively. This is a known limitation that OBS has accepted because DirectShow has the widest legacy compatibility. On Windows 11, FrameServer provides a compatibility bridge for MF-created virtual cameras to be visible to DirectShow apps, but NOT the other way around.

### Sources

- [OBS RFC #15 -- Virtual Camera Support](https://github.com/obsproject/rfcs/blob/master/accepted/0015-virtual-camera-support.md)
- [OBS PR #3087 -- Add Windows Virtual Camera](https://github.com/obsproject/obs-studio/pull/3087)
- [OBS virtualcam-module source](https://github.com/obsproject/obs-studio/blob/master/plugins/win-dshow/virtualcam-module/virtualcam-module.cpp)
- [CatxFish/obs-virtual-cam](https://github.com/CatxFish/obs-virtual-cam) (original third-party plugin)

---

## 2. How Elgato does it

### Technology

**DirectShow filter** installed as part of Elgato Camera Hub / EpocCam driver. The installer uses a standard MSI/EXE that runs elevated.

### Registration

- Camera Hub installs a DirectShow filter registered system-wide
- The "Elgato Virtual Camera" appears in all DirectShow-consuming applications
- Signed with Elgato's (Corsair's) EV code signing certificate
- EpocCam (now discontinued) also used a signed DirectShow driver

### Admin requirement

**Yes -- at install time.** Camera Hub's installer requires elevation. After install, the virtual camera works at standard user privilege.

### How it works

- Camera Hub intercepts the USB camera feed from Elgato Facecam / other hardware
- Applies processing (background blur via NVIDIA Broadcast SDK, etc.)
- Outputs processed frames to the DirectShow virtual camera filter
- Consumer apps (Zoom, Teams) see "Elgato Virtual Camera" as a camera source

### Sources

- [Elgato Camera Hub Software](https://help.elgato.com/hc/en-us/sections/360013950972-Elgato-Camera-Hub-Software)
- [EpocCam Driver Release Notes](https://help.elgato.com/hc/en-us/articles/360052826352-EpocCam-Driver-Release-Notes-Windows)

---

## 3. How GoPro Webcam Utility works

### Technology

**Native UVC (USB Video Class) -- no virtual camera at all.**

### How it works

GoPro cameras from Hero 8 onwards have a firmware "webcam mode" that, when activated via USB-C, presents the camera as a **native UVC device** to the operating system. Windows' built-in `usbvideo.inf` driver handles it directly -- no virtual camera, no DirectShow filter, no custom driver needed.

The GoPro Webcam Utility app:

1. Tells the camera firmware to switch from MTP (storage) mode to webcam mode
2. The camera then presents itself as a standard UVC device over USB
3. Windows loads the inbox USB Video Device driver (`usbvideo.inf`)
4. The camera appears as a native camera in Device Manager under "Cameras"
5. All apps see it as a real hardware camera -- MF apps, DirectShow apps, everything

### Admin requirement

**None at runtime.** The inbox UVC driver is already part of Windows. The GoPro utility just needs to trigger the firmware mode switch. The utility installer might need admin for its own app installation, but the camera itself needs zero driver installation.

### Why this works without issues

GoPro has a hardware device that speaks UVC natively (after firmware mode switch). This is fundamentally different from a software virtual camera -- there is no COM registration, no filter registration, no FrameServer involvement. The device is a real USB camera.

### Why we cannot do this

We are creating a **software** virtual camera -- there is no physical device presenting UVC frames. We need to inject frames into the OS camera pipeline from our application process. That requires either a DirectShow filter, an MF virtual camera, or a kernel driver.

### Sources

- [GoPro Webcam Information](https://community.gopro.com/s/article/GoPro-Webcam?language=en_US)
- [GoPro Webcam Fix (UVC driver)](https://github.com/eric-2812/GoPro-webcam-fix)
- [UVC Camera Implementation Guide](https://learn.microsoft.com/en-us/windows-hardware/drivers/stream/uvc-camera-implementation-guide)

---

## 4. Other notable implementations

### Snap Camera (discontinued)

- **DirectShow filter** with proprietary inter-process communication
- Installer registered the filter system-wide (admin at install time)
- Applied AR lenses to webcam feed, output to virtual camera
- Discontinued in January 2023

### ManyCam

- **DirectShow filter** ("ManyCam Virtual Webcam")
- Installer registers system-wide (admin at install time)
- Signed commercial driver
- Supports multiple simultaneous virtual camera outputs

### NDI Tools (NDI Virtual Input)

- **DirectShow filter** registered at install time
- Receives NDI network video and presents as a local camera
- Admin required for installer

### mmhmm

- **DirectShow filter** based on commercial virtual camera SDK
- Admin at install time, standard user at runtime

### PowerToys Video Conference Mute

- Initially tried MF custom media source approach
- **Switched to DirectShow** due to hardware-specific bugs, WDK dependency, and certification overhead
- Uses DirectShow filter registered at install time

### VisioForge Virtual Camera SDK

- Commercial SDK for building DirectShow virtual cameras
- Provides `regsvr32`-based registration
- Admin required for registration
- Used by many commercial products internally

### Sources

- [VisioForge Virtual Camera SDK](https://www.visioforge.com/virtual-camera-sdk)
- [PowerToys Issue #7944](https://github.com/microsoft/PowerToys/issues/7944)
- [ManyCam Virtual Webcam](https://help.manycam.com/knowledge-base/webcam-driver/)

---

## 5. The landscape of approaches on Windows

### Approach A: DirectShow virtual camera filter (legacy, most common)

**How it works:** Implement a COM DLL that exposes `IBaseFilter` + `IPin` + `IMemAllocator`. Register it via `regsvr32` in HKLM under `HKEY_CLASSES_ROOT\CLSID\{GUID}` and with DirectShow's Filter Mapper.

| Aspect             | Detail                                                                 |
| ------------------ | ---------------------------------------------------------------------- |
| OS support         | Windows 7+ (all versions)                                              |
| Admin required     | Yes, at install time (regsvr32 to HKLM)                                |
| Runtime admin      | No                                                                     |
| Visible to MF apps | **No** -- DirectShow vcams are invisible to Media Foundation consumers |
| Visible to DS apps | Yes (32-bit to 32-bit, 64-bit to 64-bit -- bitness must match)         |
| Complexity         | High (IBaseFilter, IPin, IMemAllocator, COM boilerplate)               |
| IPC for frames     | Shared memory or named pipes (DLL loads in consumer process)           |
| Crash cleanup      | Manual -- filter stays registered after crash                          |
| Used by            | OBS, Elgato, Snap Camera, ManyCam, NDI Tools, PowerToys, VisioForge    |

**Key limitation:** Modern apps increasingly use Media Foundation, not DirectShow. A DS-only virtual camera will be invisible to some MF-based apps. The OBS forums have many reports of "OBS Virtual Camera not showing up" in certain apps.

### Approach B: MFCreateVirtualCamera (Windows 11+, our current approach)

**How it works:** Call `MFCreateVirtualCamera()` to create a session-lifetime virtual camera. FrameServer loads our custom media source COM DLL and serves frames to all consumer apps.

| Aspect             | Detail                                                                                        |
| ------------------ | --------------------------------------------------------------------------------------------- |
| OS support         | Windows 11 Build 22000+ only                                                                  |
| Admin required     | Yes, for DLL registration (HKLM or MSIX COM redirection or WHQL driver)                       |
| Runtime admin      | No (once registered)                                                                          |
| Visible to MF apps | **Yes**                                                                                       |
| Visible to DS apps | **Yes** -- via FrameServer compatibility bridge (needs KSCATEGORY_VIDEO + KSCATEGORY_CAPTURE) |
| Complexity         | Medium (IMFMediaSource + COM boilerplate, simpler than DS filter)                             |
| IPC for frames     | Shared memory or file-backed (DLL runs in FrameServer, not consumer process)                  |
| Crash cleanup      | Automatic (session lifetime)                                                                  |
| Used by            | Microsoft samples (VCamSample, VCamNetSample), our project                                    |

**Key advantage over DirectShow:** Visible to BOTH MF and DirectShow apps through FrameServer. Automatic crash cleanup. Simpler COM interface to implement.

**Key pain points we hit:**

1. FrameServer (LOCAL SERVICE) cannot see HKCU COM registrations
2. `AddRegistryEntry()` returns ACCESS_DENIED for non-elevated callers
3. `AddProperty(ClassGuid)` requires elevation
4. Shared memory namespace isolation between LOCAL SERVICE and user session

### Approach C: Stub driver + Custom Media Source (pre-Windows 11 FrameServer approach)

**How it works:** Ship a WHQL-certified stub driver (UMDF or KMDF) that registers the camera device interface, plus a COM DLL for the custom media source. The driver INF handles all registration.

| Aspect             | Detail                                                                      |
| ------------------ | --------------------------------------------------------------------------- |
| OS support         | Windows 10 1809+                                                            |
| Admin required     | Yes, for driver installation                                                |
| Runtime admin      | No                                                                          |
| Visible to MF apps | Yes                                                                         |
| Visible to DS apps | Yes (if registered under KSCATEGORY_VIDEO + KSCATEGORY_CAPTURE)             |
| Complexity         | Very high (UMDF/KMDF stub driver + WHQL certification + INF file + COM DLL) |
| WHQL required      | Yes -- Microsoft requires driver packages to be WHQL-certified              |
| Used by            | Enterprise/OEM solutions, IP cameras                                        |

**Not practical for us:** WHQL certification is expensive, slow, and requires Microsoft Hardware Dev Center registration. This is the "proper" enterprise approach but massively overkill for an indie app.

### Approach D: Kernel mode AVStream minidriver (nuclear option)

**How it works:** Write a kernel-mode camera driver that creates a real device node. Visible to all APIs automatically.

| Aspect         | Detail                                                                   |
| -------------- | ------------------------------------------------------------------------ |
| OS support     | All Windows versions                                                     |
| Admin required | Yes (driver installation)                                                |
| Complexity     | Extreme (kernel development, WHQL mandatory, Secure Boot considerations) |
| Used by        | Hardware camera manufacturers                                            |

**Not practical for us.** This is for actual hardware camera vendors.

---

## 6. Why MFCreateVirtualCamera has admin issues

### The fundamental problem

`MFCreateVirtualCamera` creates a virtual camera that FrameServer (running as LOCAL SERVICE) must be able to load. This means:

1. **The COM DLL must be discoverable by LOCAL SERVICE** -- HKCU registrations are per-user and invisible to LOCAL SERVICE's different user profile
2. **The DLL file must be accessible by LOCAL SERVICE** -- user home directories are not readable by LOCAL SERVICE
3. **Device interface registration requires HKLM** -- device interfaces are system-wide, not per-user

Microsoft's documentation confirms: "it's not possible to register a Virtual Camera media source in HKCU, only in HKLM since it will be loaded by multiple processes."

### What `MFVirtualCameraAccess_CurrentUser` actually means

Despite the name, `CurrentUser` access only controls **which users can see the virtual camera in device enumeration**. It does NOT mean the COM registration can be per-user. The media source DLL still needs to be registered system-wide so FrameServer can load it.

The `AddRegistryEntry` method is supposed to help by writing the DLL path to the device node, but it returns ACCESS_DENIED for non-elevated callers because it writes to HKLM.

### Categories are not the issue

Non-admin users CAN pass `KSCATEGORY_VIDEO_CAMERA`, `KSCATEGORY_VIDEO`, `KSCATEGORY_CAPTURE`, and `KSCATEGORY_SENSOR_CAMERA` to `MFCreateVirtualCamera`. The category parameter is NOT what causes the ACCESS_DENIED. The problem is the COM registration of the media source DLL.

### What everyone else does

Every product with a virtual camera on Windows does the exact same thing: **register during installation** (which already has admin via UAC) and then run at standard user privilege. There is no magic workaround. The choice is:

1. `regsvr32` at install time (OBS, Elgato, ManyCam, etc.)
2. MSIX COM redirection registered at install time (our approach)
3. WHQL-certified stub driver installed at install time (enterprise approach)

---

## 7. Recommendation

### Keep MFCreateVirtualCamera -- it is the right choice

Our current approach is correct for these reasons:

1. **Universal visibility** -- MF virtual cameras are visible to both Media Foundation AND DirectShow apps through FrameServer. DirectShow virtual cameras are NOT visible to MF apps. As the ecosystem moves toward MF, DirectShow-only solutions become less compatible over time.

2. **Automatic crash cleanup** -- session-lifetime virtual cameras disappear when the process exits. DirectShow filters persist in the registry and require manual cleanup.

3. **Simpler implementation** -- `IMFMediaSource` is simpler than the full `IBaseFilter` + `IPin` + `IMemAllocator` DirectShow stack. We already have a working implementation.

4. **Windows 11+ only** -- our project already requires Windows 11 (Build 22000+), so the OS requirement is not a constraint.

5. **FrameServer native** -- we integrate natively with the modern camera pipeline, not through a legacy compatibility shim.

### The admin problem is already solved by our plan

Our MSIX sparse package approach (registering at install time via the NSIS installer) is the exact same pattern that OBS, Elgato, and every other product uses -- just with MSIX COM redirection instead of `regsvr32`. The NSIS installer already has UAC elevation. This is the right path.

### Do NOT switch to DirectShow

Switching to DirectShow would:

- **Lose MF app compatibility** -- growing number of modern apps use MF exclusively
- **Lose automatic crash cleanup** -- filter stays registered after crashes
- **Increase implementation complexity** -- DirectShow filter interfaces are more complex
- **Still require admin at install time** -- `regsvr32` needs HKLM access just like our MSIX approach
- **Gain nothing** -- the admin requirement is identical, the compatibility is worse

### Remaining action items

1. **Complete the MSIX sparse package implementation** -- this is the blocker. Once the NSIS installer registers the sparse package, FrameServer will see our COM class.

2. **File-backed shared memory** -- already implemented (commit db90e26). Solves the namespace isolation between LOCAL SERVICE and user session.

3. **Code signing** -- needed for MSIX package and SmartScreen. Azure Trusted Signing ($10/month) is recommended per the existing smartscreen-research.md.

4. **OBS visibility via AddProperty** -- `AddProperty(DEVPKEY_DeviceInterface_ClassGuid)` requires elevation. With MSIX sparse package, we may not need this at all because FrameServer handles device interface registration. Needs testing. If OBS still cannot see the camera, the fallback is registering under KSCATEGORY_VIDEO and KSCATEGORY_CAPTURE via the NSIS post-install hook.

---

## Appendix: Comparison matrix

| Product         | Technology            | Admin at install  | Admin at runtime | Visible to MF | Visible to DS | Crash cleanup |
| --------------- | --------------------- | ----------------- | ---------------- | ------------- | ------------- | ------------- |
| **OBS**         | DirectShow filter     | Yes (regsvr32)    | No               | **No**        | Yes           | Manual        |
| **Elgato**      | DirectShow filter     | Yes (installer)   | No               | **No**        | Yes           | Manual        |
| **GoPro**       | Native UVC (hardware) | No (inbox driver) | No               | Yes           | Yes           | N/A           |
| **Snap Camera** | DirectShow filter     | Yes (installer)   | No               | **No**        | Yes           | Manual        |
| **ManyCam**     | DirectShow filter     | Yes (installer)   | No               | **No**        | Yes           | Manual        |
| **NDI Tools**   | DirectShow filter     | Yes (installer)   | No               | **No**        | Yes           | Manual        |
| **PowerToys**   | DirectShow filter     | Yes (installer)   | No               | **No**        | Yes           | Manual        |
| **Our app**     | MFCreateVirtualCamera | Yes (MSIX sparse) | No               | **Yes**       | **Yes**       | **Automatic** |

Our approach is the only software virtual camera in this list that is visible to BOTH Media Foundation and DirectShow applications, and the only one with automatic crash cleanup. The admin requirement at install time is identical to every other product.
