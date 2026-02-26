import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { act, renderHook } from '@testing-library/react'
import { useThumbnail } from './useThumbnail.ts'

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))

import { invoke } from '@tauri-apps/api/core'
const mockInvoke = vi.mocked(invoke)

describe('useThumbnail', () => {
  beforeEach(() => {
    mockInvoke.mockReset()
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  it('returns null when deviceId is null', () => {
    const { result } = renderHook(() => useThumbnail(null))
    expect(result.current).toBeNull()
  })

  it('polls get_thumbnail at 200ms interval', async () => {
    vi.useFakeTimers()
    mockInvoke.mockResolvedValue('THUMB_DATA')

    const { result, unmount } = renderHook(() => useThumbnail('device-1'))

    await act(async () => {
      await vi.advanceTimersByTimeAsync(200)
    })

    expect(mockInvoke).toHaveBeenCalledWith('get_thumbnail', {
      deviceId: 'device-1',
    })
    expect(result.current).toBe('data:image/jpeg;base64,THUMB_DATA')

    unmount()
    vi.useRealTimers()
  })

  it('cleans up interval on unmount', async () => {
    vi.useFakeTimers()
    mockInvoke.mockResolvedValue('DATA')

    const { unmount } = renderHook(() => useThumbnail('device-1'))
    unmount()

    mockInvoke.mockClear()
    await act(async () => {
      await vi.advanceTimersByTimeAsync(400)
    })

    expect(mockInvoke).not.toHaveBeenCalled()

    vi.useRealTimers()
  })
})
