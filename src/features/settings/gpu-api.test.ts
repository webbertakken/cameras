import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { GpuAdapterInfo } from '../../types/gpu'
import { getActiveGpu, listGpuAdapters, setGpuAdapter } from './gpu-api'

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))

const { invoke } = await import('@tauri-apps/api/core')
const mockInvoke = vi.mocked(invoke)

const testAdapter: GpuAdapterInfo = {
  index: 0,
  name: 'NVIDIA GeForce RTX 3080',
  backend: 'Vulkan',
  deviceType: 'DiscreteGpu',
}

describe('GPU API', () => {
  beforeEach(() => {
    mockInvoke.mockReset()
  })

  it('lists available GPU adapters', async () => {
    mockInvoke.mockResolvedValueOnce([testAdapter])
    const result = await listGpuAdapters()
    expect(mockInvoke).toHaveBeenCalledWith('list_gpu_adapters')
    expect(result).toEqual([testAdapter])
  })

  it('gets active GPU adapter', async () => {
    mockInvoke.mockResolvedValueOnce(testAdapter)
    const result = await getActiveGpu()
    expect(mockInvoke).toHaveBeenCalledWith('get_active_gpu')
    expect(result).toEqual(testAdapter)
  })

  it('returns null when no active GPU', async () => {
    mockInvoke.mockResolvedValueOnce(null)
    const result = await getActiveGpu()
    expect(result).toBeNull()
  })

  it('sets GPU adapter by index', async () => {
    mockInvoke.mockResolvedValueOnce('NVIDIA GeForce RTX 3080')
    const result = await setGpuAdapter(0)
    expect(mockInvoke).toHaveBeenCalledWith('set_gpu_adapter', { adapterIndex: 0 })
    expect(result).toBe('NVIDIA GeForce RTX 3080')
  })

  it('disables GPU with null adapter index', async () => {
    mockInvoke.mockResolvedValueOnce(null)
    const result = await setGpuAdapter(null)
    expect(mockInvoke).toHaveBeenCalledWith('set_gpu_adapter', { adapterIndex: null })
    expect(result).toBeNull()
  })

  it('propagates backend errors', async () => {
    mockInvoke.mockRejectedValueOnce(new Error('GPU init failed'))
    await expect(listGpuAdapters()).rejects.toThrow('GPU init failed')
  })
})
