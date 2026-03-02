import { invoke } from '@tauri-apps/api/core'
import type { GpuAdapterInfo } from '../../types/gpu'

/** List all available GPU adapters on the system. */
export async function listGpuAdapters(): Promise<GpuAdapterInfo[]> {
  return invoke<GpuAdapterInfo[]>('list_gpu_adapters')
}

/** Get the currently active GPU adapter, or null if in CPU-only mode. */
export async function getActiveGpu(): Promise<GpuAdapterInfo | null> {
  return invoke<GpuAdapterInfo | null>('get_active_gpu')
}

/**
 * Switch the active GPU adapter.
 * Pass `null` to disable GPU acceleration (CPU-only mode).
 * Returns the name of the selected adapter, or null if disabled/failed.
 */
export async function setGpuAdapter(adapterIndex: number | null): Promise<string | null> {
  return invoke<string | null>('set_gpu_adapter', { adapterIndex })
}
