import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { act, renderHook } from '@testing-library/react'
import { useDiagnostics } from './useDiagnostics.ts'

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))

import { invoke } from '@tauri-apps/api/core'
const mockInvoke = vi.mocked(invoke)

const mockDiagnostics = {
  fps: 29.97,
  frameCount: 300,
  dropCount: 2,
  dropRate: 0.66,
  latencyMs: 12.5,
  bandwidthBps: 5_000_000,
  usbBusInfo: null,
}

const mockEncoding = {
  encoderKind: 'mfSoftware',
  framesEncoded: 500,
  framesDropped: 1,
  avgEncodeMs: 3.2,
  lastEncodeMs: 2.8,
}

describe('useDiagnostics', () => {
  beforeEach(() => {
    mockInvoke.mockReset()
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  it('returns null when not enabled', () => {
    const { result } = renderHook(() => useDiagnostics('device-1', false))
    expect(result.current).toBeNull()
  })

  it('returns null when deviceId is null', () => {
    const { result } = renderHook(() => useDiagnostics(null, true))
    expect(result.current).toBeNull()
  })

  it('polls both diagnostics and encoding stats at 1000ms interval', async () => {
    vi.useFakeTimers()
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_diagnostics') return Promise.resolve(mockDiagnostics)
      if (cmd === 'get_encoding_stats') return Promise.resolve(mockEncoding)
      return Promise.resolve(undefined)
    })

    const { result, unmount } = renderHook(() => useDiagnostics('device-1', true))

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000)
    })

    expect(mockInvoke).toHaveBeenCalledWith('get_diagnostics', { deviceId: 'device-1' })
    expect(mockInvoke).toHaveBeenCalledWith('get_encoding_stats', { deviceId: 'device-1' })
    expect(result.current).toEqual({
      diagnostics: mockDiagnostics,
      encoding: mockEncoding,
    })

    unmount()
    vi.useRealTimers()
  })

  it('returns encoding as null when get_encoding_stats fails', async () => {
    vi.useFakeTimers()
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_diagnostics') return Promise.resolve(mockDiagnostics)
      if (cmd === 'get_encoding_stats') return Promise.reject(new Error('Not available'))
      return Promise.resolve(undefined)
    })

    const { result, unmount } = renderHook(() => useDiagnostics('device-1', true))

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000)
    })

    expect(result.current).toEqual({
      diagnostics: mockDiagnostics,
      encoding: null,
    })

    unmount()
    vi.useRealTimers()
  })

  it('cleans up interval when disabled', async () => {
    vi.useFakeTimers()
    mockInvoke.mockResolvedValue(mockDiagnostics)

    const { rerender, unmount } = renderHook(({ enabled }) => useDiagnostics('device-1', enabled), {
      initialProps: { enabled: true },
    })

    rerender({ enabled: false })
    mockInvoke.mockClear()

    await act(async () => {
      await vi.advanceTimersByTimeAsync(2000)
    })

    expect(mockInvoke).not.toHaveBeenCalled()

    unmount()
    vi.useRealTimers()
  })
})
