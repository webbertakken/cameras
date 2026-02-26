import { renderHook } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { useCameraStore } from './store'

const mockUnlisten = vi.fn()
const mockOnCameraHotplug = vi.fn()

vi.mock('./api', () => ({
  onCameraHotplug: (...args: unknown[]) => mockOnCameraHotplug(...args),
}))

import { useHotplug } from './useHotplug'

describe('useHotplug', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mockOnCameraHotplug.mockResolvedValue(mockUnlisten)
    useCameraStore.setState({ cameras: [], selectedId: null })
  })

  it('subscribes to camera-hotplug events on mount', () => {
    renderHook(() => useHotplug())
    expect(mockOnCameraHotplug).toHaveBeenCalledWith(expect.any(Function))
  })

  it('unsubscribes on unmount', async () => {
    const { unmount } = renderHook(() => useHotplug())

    // Wait for the promise in the effect to resolve
    await vi.waitFor(() => {
      expect(mockOnCameraHotplug).toHaveBeenCalled()
    })

    unmount()
    expect(mockUnlisten).toHaveBeenCalled()
  })

  it('calls addCamera on connected event', async () => {
    mockOnCameraHotplug.mockImplementation((callback: (event: unknown) => void) => {
      callback({
        type: 'connected',
        id: 'cam-1',
        name: 'Webcam',
        devicePath: '/dev/video0',
        isConnected: true,
      })
      return Promise.resolve(mockUnlisten)
    })

    renderHook(() => useHotplug())

    expect(useCameraStore.getState().cameras).toEqual([
      { id: 'cam-1', name: 'Webcam', devicePath: '/dev/video0', isConnected: true },
    ])
  })

  it('calls removeCamera on disconnected event', () => {
    useCameraStore.setState({
      cameras: [{ id: 'cam-1', name: 'Webcam', devicePath: '/dev/video0', isConnected: true }],
    })

    mockOnCameraHotplug.mockImplementation((callback: (event: unknown) => void) => {
      callback({ type: 'disconnected', id: 'cam-1' })
      return Promise.resolve(mockUnlisten)
    })

    renderHook(() => useHotplug())

    expect(useCameraStore.getState().cameras).toEqual([])
  })
})
