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
