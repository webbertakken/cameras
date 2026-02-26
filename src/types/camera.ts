/** Camera device as serialised from the Rust backend (camelCase). */
export interface CameraDevice {
  id: string
  name: string
  devicePath: string
  isConnected: boolean
}

/** Hot-plug event emitted by the `camera-hotplug` Tauri event. */
export interface HotplugEvent {
  type: 'connected' | 'disconnected'
  id: string
  name?: string
  devicePath?: string
  isConnected?: boolean
}

/** Type of UI control widget — matches Rust ControlType. */
export type ControlType = 'slider' | 'toggle' | 'select'

/** Control group for accordion sections — matches Rust ControlId::group(). */
export type ControlGroup = 'image' | 'exposure' | 'focus' | 'advanced'

/** Control capability flags — matches Rust ControlFlags. */
export interface ControlFlags {
  supportsAuto: boolean
  isAutoEnabled: boolean
  isReadOnly: boolean
}

/** Full metadata for a single camera control — matches Rust ControlDescriptor. */
export interface ControlDescriptor {
  id: string
  name: string
  controlType: ControlType
  group: ControlGroup
  min: number | null
  max: number | null
  step: number | null
  default: number | null
  current: number
  flags: ControlFlags
  supported: boolean
}
