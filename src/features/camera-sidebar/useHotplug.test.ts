import { renderHook } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { useToastStore } from '../notifications/useToast'
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
    useToastStore.setState({ toasts: [] })
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

  it('shows success toast on camera connected', () => {
    mockOnCameraHotplug.mockImplementation((callback: (event: unknown) => void) => {
      callback({
        type: 'connected',
        id: 'cam-1',
        name: 'Logitech C920',
        devicePath: '/dev/video0',
        isConnected: true,
      })
      return Promise.resolve(mockUnlisten)
    })

    renderHook(() => useHotplug())

    const toasts = useToastStore.getState().toasts
    expect(toasts).toHaveLength(1)
    expect(toasts[0]).toMatchObject({
      message: 'Logitech C920 connected',
      type: 'success',
    })
  })

  it('shows info toast on camera disconnected with camera name', () => {
    useCameraStore.setState({
      cameras: [
        { id: 'cam-1', name: 'Logitech C920', devicePath: '/dev/video0', isConnected: true },
      ],
    })

    mockOnCameraHotplug.mockImplementation((callback: (event: unknown) => void) => {
      callback({ type: 'disconnected', id: 'cam-1' })
      return Promise.resolve(mockUnlisten)
    })

    renderHook(() => useHotplug())

    const toasts = useToastStore.getState().toasts
    expect(toasts).toHaveLength(1)
    expect(toasts[0]).toMatchObject({
      message: 'Logitech C920 disconnected',
      type: 'info',
    })
  })

  it('uses fallback name for unknown disconnected camera', () => {
    mockOnCameraHotplug.mockImplementation((callback: (event: unknown) => void) => {
      callback({ type: 'disconnected', id: 'unknown-cam' })
      return Promise.resolve(mockUnlisten)
    })

    renderHook(() => useHotplug())

    const toasts = useToastStore.getState().toasts
    expect(toasts).toHaveLength(1)
    expect(toasts[0]).toMatchObject({
      message: 'Camera disconnected',
      type: 'info',
    })
  })
})
