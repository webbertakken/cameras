import { type Mock, beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('./virtual-camera-api', () => ({
  startVirtualCamera: vi.fn(),
  stopVirtualCamera: vi.fn(),
  getVirtualCameraStatus: vi.fn(),
}))

import { startVirtualCamera, stopVirtualCamera } from './virtual-camera-api'
import { useVirtualCameraStore } from './virtual-camera-store'

describe('useVirtualCameraStore', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    useVirtualCameraStore.setState({ activeDevices: new Set() })
  })

  it('tracks active devices', () => {
    const store = useVirtualCameraStore.getState()
    expect(store.isActive('cam-1')).toBe(false)

    store.setActive('cam-1', true)
    expect(useVirtualCameraStore.getState().isActive('cam-1')).toBe(true)

    store.setActive('cam-1', false)
    expect(useVirtualCameraStore.getState().isActive('cam-1')).toBe(false)
  })

  it('toggle calls startVirtualCamera when inactive', async () => {
    ;(startVirtualCamera as Mock).mockResolvedValue(undefined)

    await useVirtualCameraStore.getState().toggle('cam-1')

    expect(startVirtualCamera).toHaveBeenCalledWith('cam-1')
    expect(useVirtualCameraStore.getState().isActive('cam-1')).toBe(true)
  })

  it('toggle calls stopVirtualCamera when active', async () => {
    ;(stopVirtualCamera as Mock).mockResolvedValue(undefined)
    useVirtualCameraStore.setState({ activeDevices: new Set(['cam-1']) })

    await useVirtualCameraStore.getState().toggle('cam-1')

    expect(stopVirtualCamera).toHaveBeenCalledWith('cam-1')
    expect(useVirtualCameraStore.getState().isActive('cam-1')).toBe(false)
  })

  it('toggle does not update state when start fails', async () => {
    ;(startVirtualCamera as Mock).mockRejectedValue(new Error('sink failed'))
    const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {})

    await expect(useVirtualCameraStore.getState().toggle('cam-1')).rejects.toThrow('sink failed')

    expect(useVirtualCameraStore.getState().isActive('cam-1')).toBe(false)
    expect(consoleSpy).toHaveBeenCalledWith(
      expect.stringContaining('Virtual camera toggle failed'),
      expect.any(Error),
    )
    consoleSpy.mockRestore()
  })

  it('toggle does not update state when stop fails', async () => {
    ;(stopVirtualCamera as Mock).mockRejectedValue(new Error('release failed'))
    useVirtualCameraStore.setState({ activeDevices: new Set(['cam-1']) })
    const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {})

    await expect(useVirtualCameraStore.getState().toggle('cam-1')).rejects.toThrow('release failed')

    // Device should still be marked active since stop failed
    expect(useVirtualCameraStore.getState().isActive('cam-1')).toBe(true)
    expect(consoleSpy).toHaveBeenCalledWith(
      expect.stringContaining('Virtual camera toggle failed'),
      expect.any(Error),
    )
    consoleSpy.mockRestore()
  })

  it('setActive adds and removes device IDs', () => {
    const store = useVirtualCameraStore.getState()

    store.setActive('cam-1', true)
    store.setActive('cam-2', true)
    expect(useVirtualCameraStore.getState().isActive('cam-1')).toBe(true)
    expect(useVirtualCameraStore.getState().isActive('cam-2')).toBe(true)

    store.setActive('cam-1', false)
    expect(useVirtualCameraStore.getState().isActive('cam-1')).toBe(false)
    expect(useVirtualCameraStore.getState().isActive('cam-2')).toBe(true)
  })
})
