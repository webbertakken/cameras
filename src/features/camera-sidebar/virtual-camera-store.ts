import { create } from 'zustand'
import { startVirtualCamera, stopVirtualCamera } from './virtual-camera-api'

interface VirtualCameraStore {
  /** Device IDs with an active virtual camera output. */
  activeDevices: Set<string>
  /** Toggle the virtual camera for a device on or off. */
  toggle: (deviceId: string) => Promise<void>
  /** Check whether a virtual camera is active for a device. */
  isActive: (deviceId: string) => boolean
  /** Set the active state for a device (used for syncing from backend). */
  setActive: (deviceId: string, active: boolean) => void
}

export const useVirtualCameraStore = create<VirtualCameraStore>((set, get) => ({
  activeDevices: new Set(),

  toggle: async (deviceId) => {
    const { activeDevices } = get()
    const wasActive = activeDevices.has(deviceId)
    const action = wasActive ? 'stop' : 'start'
    try {
      console.info(`[vcam-store] Sending IPC '${action}_virtual_camera' for '${deviceId}'`)
      if (wasActive) {
        await stopVirtualCamera(deviceId)
        set((state) => {
          const next = new Set(state.activeDevices)
          next.delete(deviceId)
          return { activeDevices: next }
        })
      } else {
        await startVirtualCamera(deviceId)
        set((state) => {
          const next = new Set(state.activeDevices)
          next.add(deviceId)
          return { activeDevices: next }
        })
      }
      console.info(`[vcam-store] IPC '${action}_virtual_camera' succeeded for '${deviceId}'`)
    } catch (err) {
      console.error(`[vcam-store] IPC '${action}_virtual_camera' failed for '${deviceId}':`, err)
      throw err
    }
  },

  isActive: (deviceId) => get().activeDevices.has(deviceId),

  setActive: (deviceId, active) =>
    set((state) => {
      const next = new Set(state.activeDevices)
      if (active) {
        next.add(deviceId)
      } else {
        next.delete(deviceId)
      }
      return { activeDevices: next }
    }),
}))
