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
    if (activeDevices.has(deviceId)) {
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
