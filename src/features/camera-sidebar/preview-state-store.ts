import { invoke } from '@tauri-apps/api/core'
import { create } from 'zustand'

interface PreviewStateStore {
  /** Device IDs with active backend preview sessions. */
  activeDeviceIds: Set<string>
  /** Fetch active preview device IDs from the backend. */
  refresh: () => Promise<void>
}

export const usePreviewStateStore = create<PreviewStateStore>((set) => ({
  activeDeviceIds: new Set(),

  refresh: async () => {
    const ids = await invoke<string[]>('get_active_previews')
    set({ activeDeviceIds: new Set(ids) })
  },
}))
