## ADDED requirements

### Requirement: Canon camera connect/disconnect events

The system SHALL detect Canon camera connection and disconnection via EDSDK camera state events and emit `HotplugEvent` values through the existing hotplug system.

**Context**: EDSDK provides `EdsSetCameraStateEventHandler()` which fires `kEdsStateEvent_Shutdown` when a camera disconnects. New camera connections are detected by periodic re-enumeration (EDSDK has no connection event — only disconnection).

#### Scenario: Canon camera disconnected

- **WHEN** a Canon camera is unplugged while the app is running
- **THEN** `kEdsStateEvent_Shutdown` fires on the EDSDK event handler
- **AND** a `HotplugEvent::Disconnected` is emitted within 3 seconds
- **AND** the camera is removed from the sidebar

#### Scenario: Canon camera connected

- **WHEN** a Canon camera is plugged in while the app is running
- **THEN** periodic re-enumeration detects the new camera within 5 seconds
- **AND** a `HotplugEvent::Connected` is emitted
- **AND** the camera appears in the sidebar

#### Scenario: EDSDK event processing

- **WHEN** EDSDK events are registered
- **THEN** `EdsGetEvent()` is called periodically on the EDSDK thread to process pending events

### Requirement: Clean session teardown on disconnect

The system SHALL close the EDSDK session and release camera refs when a disconnect event is received, preventing resource leaks.

#### Scenario: Disconnect during live view

- **WHEN** a Canon camera disconnects while live view is active
- **THEN** the live view polling thread is stopped
- **AND** the EDSDK session is closed
- **AND** camera refs are released

## Technical notes

- Hotplug in `src-tauri/src/camera/canon/hotplug.rs`
- EDSDK state events: `kEdsStateEvent_Shutdown` (disconnect), `kEdsStateEvent_WillSoonShutDown` (battery warning)
- Re-enumeration interval for new connections: 3-5 seconds
- The EDSDK event loop and re-enumeration can share a thread with periodic `EdsGetEvent()` + `EdsGetCameraList()` calls
