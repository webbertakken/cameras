import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { act, renderHook } from '@testing-library/react'
import { usePreview } from './usePreview.ts'

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))

import { invoke } from '@tauri-apps/api/core'
const mockInvoke = vi.mocked(invoke)

describe('usePreview', () => {
  beforeEach(() => {
    mockInvoke.mockReset()
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  it('initialises with null frame and inactive state', () => {
    const { result } = renderHook(() => usePreview('device-1'))
    expect(result.current.frameSrc).toBeNull()
    expect(result.current.isActive).toBe(false)
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

    // Clean up — stop the interval
    await act(async () => {
      await result.current.stop()
    })
  })

  it('calls stop_preview on stop', async () => {
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

  it('sets frameSrc after polling get_frame', async () => {
    vi.useFakeTimers()
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'start_preview') return undefined
      if (cmd === 'get_frame') return 'AQID'
      if (cmd === 'stop_preview') return undefined
      return undefined
    })

    const { result } = renderHook(() => usePreview('device-1'))

    await act(async () => {
      await result.current.start(640, 480, 30)
    })

    await act(async () => {
      await vi.advanceTimersByTimeAsync(50)
    })

    expect(result.current.frameSrc).toBe('data:image/jpeg;base64,AQID')

    await act(async () => {
      await result.current.stop()
    })

    vi.useRealTimers()
  })
})
