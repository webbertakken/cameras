import { renderHook } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { useToastStore } from '../notifications/useToast'
import { useCameraStore } from './store'

const mockUnlisten = vi.fn()
const mockOnCameraHotplug = vi.fn()
const mockUnlistenSettings = vi.fn()

vi.mock('./api', () => ({
  onCameraHotplug: (...args: unknown[]) => mockOnCameraHotplug(...args),
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(),
}))

const { listen } = await import('@tauri-apps/api/event')
const mockListen = vi.mocked(listen)

import { useHotplug } from './useHotplug'

describe('useHotplug', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mockOnCameraHotplug.mockResolvedValue(mockUnlisten)
    mockListen.mockResolvedValue(mockUnlistenSettings)
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

  // --- Settings restored ---

  it('subscribes to settings-restored events on mount', () => {
    renderHook(() => useHotplug())
    expect(mockListen).toHaveBeenCalledWith('settings-restored', expect.any(Function))
  })

  it('shows success toast when settings-restored event received with controlsApplied > 0', async () => {
    mockListen.mockImplementation(((
      event: string,
      callback: (event: { payload: unknown }) => void,
    ) => {
      if (event === 'settings-restored') {
        callback({
          payload: {
            deviceId: 'cam-1',
            cameraName: 'Logitech C920',
            controlsApplied: 5,
          },
        })
      }
      return Promise.resolve(mockUnlistenSettings)
    }) as unknown as typeof listen)

    renderHook(() => useHotplug())

    const toasts = useToastStore.getState().toasts
    const settingsToast = toasts.find((t) => t.message.includes('Settings restored'))
    expect(settingsToast).toBeDefined()
    expect(settingsToast).toMatchObject({ type: 'success' })
  })

  it('does not show toast when controlsApplied is 0', () => {
    mockListen.mockImplementation(((
      event: string,
      callback: (event: { payload: unknown }) => void,
    ) => {
      if (event === 'settings-restored') {
        callback({
          payload: {
            deviceId: 'cam-1',
            cameraName: 'Logitech C920',
            controlsApplied: 0,
          },
        })
      }
      return Promise.resolve(mockUnlistenSettings)
    }) as unknown as typeof listen)

    renderHook(() => useHotplug())

    const toasts = useToastStore.getState().toasts
    const settingsToast = toasts.find((t) => t.message.includes('Settings restored'))
    expect(settingsToast).toBeUndefined()
  })

  it('unsubscribes from settings-restored on unmount', async () => {
    const { unmount } = renderHook(() => useHotplug())

    await vi.waitFor(() => {
      expect(mockListen).toHaveBeenCalled()
    })

    unmount()
    expect(mockUnlistenSettings).toHaveBeenCalled()
  })
})
