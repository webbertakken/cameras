import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { act, renderHook } from '@testing-library/react'
import { usePreview } from './usePreview.ts'

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(),
}))

import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
const mockInvoke = vi.mocked(invoke)
const mockListen = vi.mocked(listen)

// Mock URL.createObjectURL / revokeObjectURL
const mockCreateObjectURL = vi.fn()
const mockRevokeObjectURL = vi.fn()
globalThis.URL.createObjectURL = mockCreateObjectURL
globalThis.URL.revokeObjectURL = mockRevokeObjectURL

describe('usePreview', () => {
  const mockUnlisten = vi.fn()

  beforeEach(() => {
    mockInvoke.mockReset()
    mockCreateObjectURL.mockReset()
    mockRevokeObjectURL.mockReset()
    mockUnlisten.mockReset()
    mockListen.mockReset()
    mockListen.mockResolvedValue(mockUnlisten)
    mockCreateObjectURL.mockReturnValue('blob:http://localhost/fake-blob')
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  it('initialises with null frame, inactive state, and no error', () => {
    const { result } = renderHook(() => usePreview('device-1'))
    expect(result.current.frameSrc).toBeNull()
    expect(result.current.isActive).toBe(false)
    expect(result.current.error).toBeNull()
  })

  it('handles null deviceId gracefully', async () => {
    const { result } = renderHook(() => usePreview(null))
    await act(async () => {
      await result.current.start(640, 480, 30)
    })
    expect(mockInvoke).not.toHaveBeenCalled()
  })

  it('calls start_preview on start', async () => {
    mockInvoke.mockResolvedValue(undefined)
    const { result } = renderHook(() => usePreview('device-1'))

    await act(async () => {
      await result.current.start(1920, 1080, 30)
    })

    expect(mockInvoke).toHaveBeenCalledWith('start_preview', {
      deviceId: 'device-1',
      width: 1920,
      height: 1080,
      fps: 30,
    })
    expect(result.current.isActive).toBe(true)

    // Clean up
    await act(async () => {
      await result.current.stop()
    })
  })

  it('calls stop_preview on stop and clears frameSrc', async () => {
    mockInvoke.mockResolvedValue(undefined)
    const { result } = renderHook(() => usePreview('device-1'))

    await act(async () => {
      await result.current.start(640, 480, 30)
    })
    await act(async () => {
      await result.current.stop()
    })

    expect(mockInvoke).toHaveBeenCalledWith('stop_preview', {
      deviceId: 'device-1',
    })
    expect(result.current.isActive).toBe(false)
    expect(result.current.frameSrc).toBeNull()
  })

  it('creates blob URL from base64 frame', async () => {
    const fakeBase64 = btoa(String.fromCharCode(0xff, 0xd8, 0x01, 0x02))
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'start_preview') return undefined
      if (cmd === 'get_frame') return fakeBase64
      if (cmd === 'stop_preview') return undefined
      return undefined
    })

    const rafCallbacks: FrameRequestCallback[] = []
    vi.spyOn(globalThis, 'requestAnimationFrame').mockImplementation((cb) => {
      rafCallbacks.push(cb)
      return rafCallbacks.length
    })

    const { result } = renderHook(() => usePreview('device-1'))

    await act(async () => {
      await result.current.start(640, 480, 30)
    })

    // Trigger one rAF callback
    await act(async () => {
      if (rafCallbacks.length > 0) {
        await rafCallbacks[rafCallbacks.length - 1](performance.now())
      }
    })

    expect(mockCreateObjectURL).toHaveBeenCalled()
    expect(result.current.frameSrc).toBe('blob:http://localhost/fake-blob')

    await act(async () => {
      await result.current.stop()
    })
  })

  it('revokes previous blob URL when creating new one', async () => {
    let frameCount = 0
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'start_preview') return undefined
      if (cmd === 'get_frame') return btoa(String.fromCharCode(0xff, 0xd8, ++frameCount))
      if (cmd === 'stop_preview') return undefined
      return undefined
    })

    mockCreateObjectURL
      .mockReturnValueOnce('blob:http://localhost/frame-1')
      .mockReturnValueOnce('blob:http://localhost/frame-2')

    const rafCallbacks: FrameRequestCallback[] = []
    vi.spyOn(globalThis, 'requestAnimationFrame').mockImplementation((cb) => {
      rafCallbacks.push(cb)
      return rafCallbacks.length
    })

    const { result } = renderHook(() => usePreview('device-1'))

    await act(async () => {
      await result.current.start(640, 480, 30)
    })

    // First frame
    await act(async () => {
      await rafCallbacks[rafCallbacks.length - 1](performance.now())
    })

    // Second frame — should revoke the first blob URL
    await act(async () => {
      await rafCallbacks[rafCallbacks.length - 1](performance.now())
    })

    expect(mockRevokeObjectURL).toHaveBeenCalledWith('blob:http://localhost/frame-1')

    await act(async () => {
      await result.current.stop()
    })
  })

  it('cancels existing loop when start is called again', async () => {
    const cancelSpy = vi.spyOn(globalThis, 'cancelAnimationFrame')
    mockInvoke.mockResolvedValue(undefined)

    vi.spyOn(globalThis, 'requestAnimationFrame').mockReturnValue(42)

    const { result } = renderHook(() => usePreview('device-1'))

    await act(async () => {
      await result.current.start(640, 480, 30)
    })

    // Start again — should cancel previous rAF
    await act(async () => {
      await result.current.start(640, 480, 30)
    })

    expect(cancelSpy).toHaveBeenCalled()

    await act(async () => {
      await result.current.stop()
    })
  })

  it('sets error state when start_preview fails', async () => {
    mockInvoke.mockRejectedValue(new Error('Device busy'))

    const { result } = renderHook(() => usePreview('device-1'))

    await act(async () => {
      await result.current.start(640, 480, 30)
    })

    expect(result.current.error).toBe('Device busy')
    expect(result.current.isActive).toBe(false)
  })

  it('listens for preview-error events', () => {
    renderHook(() => usePreview('device-1'))

    expect(mockListen).toHaveBeenCalledWith('preview-error', expect.any(Function))
  })

  it('sets error when preview-error event matches deviceId', async () => {
    type PreviewErrorPayload = { deviceId: string; error: string }
    type EventHandler = (event: { event: string; id: number; payload: PreviewErrorPayload }) => void
    let errorCallback: EventHandler | null = null
    mockListen.mockImplementation(async (_event, cb) => {
      errorCallback = cb as unknown as EventHandler
      return mockUnlisten
    })

    const { result } = renderHook(() => usePreview('device-1'))

    // Wait for effect to run
    await act(async () => {
      // Allow useEffect to complete
    })

    // Simulate a preview-error event
    await act(async () => {
      errorCallback?.({
        event: 'preview-error',
        id: 1,
        payload: { deviceId: 'device-1', error: 'Camera disconnected' },
      })
    })

    expect(result.current.error).toBe('Camera disconnected')
    expect(result.current.frameSrc).toBeNull()
  })

  it('ignores preview-error events for other devices', async () => {
    type PreviewErrorPayload = { deviceId: string; error: string }
    type EventHandler = (event: { event: string; id: number; payload: PreviewErrorPayload }) => void
    let errorCallback: EventHandler | null = null
    mockListen.mockImplementation(async (_event, cb) => {
      errorCallback = cb as unknown as EventHandler
      return mockUnlisten
    })

    const { result } = renderHook(() => usePreview('device-1'))

    await act(async () => {})

    await act(async () => {
      errorCallback?.({
        event: 'preview-error',
        id: 2,
        payload: { deviceId: 'device-2', error: 'Some error' },
      })
    })

    expect(result.current.error).toBeNull()
  })

  it('clears error when start is called successfully', async () => {
    mockInvoke.mockRejectedValueOnce(new Error('Device busy')).mockResolvedValue(undefined)

    const { result } = renderHook(() => usePreview('device-1'))

    await act(async () => {
      await result.current.start(640, 480, 30)
    })

    expect(result.current.error).toBe('Device busy')

    await act(async () => {
      await result.current.start(640, 480, 30)
    })

    expect(result.current.error).toBeNull()
    expect(result.current.isActive).toBe(true)

    await act(async () => {
      await result.current.stop()
    })
  })

  it('does not count failures during the startup grace period', async () => {
    const now = vi.spyOn(Date, 'now')
    const startTime = 1000
    now.mockReturnValue(startTime)

    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'start_preview') return undefined
      if (cmd === 'get_frame') throw new Error('no frame available')
      if (cmd === 'stop_preview') return undefined
      return undefined
    })

    const rafCallbacks: FrameRequestCallback[] = []
    vi.spyOn(globalThis, 'requestAnimationFrame').mockImplementation((cb) => {
      rafCallbacks.push(cb)
      return rafCallbacks.length
    })

    const { result } = renderHook(() => usePreview('device-1'))

    await act(async () => {
      await result.current.start(640, 480, 30)
    })

    // Simulate 200 failures while still within grace period (4.9s after start)
    now.mockReturnValue(startTime + 4900)
    for (let i = 0; i < 200; i++) {
      await act(async () => {
        const cb = rafCallbacks[rafCallbacks.length - 1]
        if (cb) await cb(performance.now())
      })
    }

    // Should still be active — grace period protects against failure counting
    expect(result.current.error).toBeNull()
    expect(result.current.isActive).toBe(true)

    await act(async () => {
      await result.current.stop()
    })

    now.mockRestore()
  })

  it('stops after consecutive failures once grace period has elapsed', async () => {
    const now = vi.spyOn(Date, 'now')
    const startTime = 1000
    now.mockReturnValue(startTime)

    let callCount = 0
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'start_preview') return undefined
      if (cmd === 'get_frame') {
        callCount++
        throw new Error('no frame available')
      }
      if (cmd === 'stop_preview') return undefined
      return undefined
    })

    const rafCallbacks: FrameRequestCallback[] = []
    vi.spyOn(globalThis, 'requestAnimationFrame').mockImplementation((cb) => {
      rafCallbacks.push(cb)
      return rafCallbacks.length
    })

    const { result } = renderHook(() => usePreview('device-1'))

    await act(async () => {
      await result.current.start(640, 480, 30)
    })

    // Move past grace period (5s)
    now.mockReturnValue(startTime + 5001)

    // Trigger 150 consecutive failures (MAX_CONSECUTIVE_FAILURES)
    for (let i = 0; i < 150; i++) {
      await act(async () => {
        const cb = rafCallbacks[rafCallbacks.length - 1]
        if (cb) await cb(performance.now())
      })
    }

    expect(callCount).toBe(150)
    expect(result.current.error).toBe('Preview stopped — camera is not producing frames')
    expect(result.current.isActive).toBe(false)

    now.mockRestore()
  })

  it('resets failure counter on successful frame', async () => {
    const now = vi.spyOn(Date, 'now')
    const startTime = 1000
    now.mockReturnValue(startTime)

    let callCount = 0
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'start_preview') return undefined
      if (cmd === 'get_frame') {
        callCount++
        // Fail for 149 calls, then succeed, then fail again
        if (callCount <= 149 || callCount === 151) {
          throw new Error('no frame available')
        }
        return btoa(String.fromCharCode(0xff, 0xd8, 0x01))
      }
      if (cmd === 'stop_preview') return undefined
      return undefined
    })

    const rafCallbacks: FrameRequestCallback[] = []
    vi.spyOn(globalThis, 'requestAnimationFrame').mockImplementation((cb) => {
      rafCallbacks.push(cb)
      return rafCallbacks.length
    })

    const { result } = renderHook(() => usePreview('device-1'))

    await act(async () => {
      await result.current.start(640, 480, 30)
    })

    // Move past grace period
    now.mockReturnValue(startTime + 6000)

    // 149 failures — should not trigger error yet
    for (let i = 0; i < 149; i++) {
      await act(async () => {
        const cb = rafCallbacks[rafCallbacks.length - 1]
        if (cb) await cb(performance.now())
      })
    }

    expect(result.current.error).toBeNull()
    expect(result.current.isActive).toBe(true)

    // 150th call succeeds — counter resets
    await act(async () => {
      const cb = rafCallbacks[rafCallbacks.length - 1]
      if (cb) await cb(performance.now())
    })

    expect(result.current.error).toBeNull()
    expect(result.current.isActive).toBe(true)

    await act(async () => {
      await result.current.stop()
    })

    now.mockRestore()
  })

  it('unlistens for preview-error on unmount', async () => {
    mockListen.mockResolvedValue(mockUnlisten)

    const { unmount } = renderHook(() => usePreview('device-1'))

    // Wait for the listen promise to resolve inside the effect
    await act(async () => {})

    unmount()

    // The cleanup calls unlistenPromise.then(fn => fn()) which is async
    // Flush the microtask queue so the then() callback executes
    await act(async () => {})

    expect(mockUnlisten).toHaveBeenCalled()
  })
})
