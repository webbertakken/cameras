import { beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))

import { invoke } from '@tauri-apps/api/core'
import { usePreviewStateStore } from './preview-state-store'

describe('usePreviewStateStore', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    usePreviewStateStore.setState({ activeDeviceIds: new Set() })
  })

  it('starts with empty active devices', () => {
    const { activeDeviceIds } = usePreviewStateStore.getState()
    expect(activeDeviceIds.size).toBe(0)
  })

  it('refresh populates active device IDs from backend', async () => {
    vi.mocked(invoke).mockResolvedValue(['cam-1', 'cam-2'])

    await usePreviewStateStore.getState().refresh()

    const { activeDeviceIds } = usePreviewStateStore.getState()
    expect(activeDeviceIds.has('cam-1')).toBe(true)
    expect(activeDeviceIds.has('cam-2')).toBe(true)
    expect(activeDeviceIds.size).toBe(2)
    expect(invoke).toHaveBeenCalledWith('get_active_previews')
  })

  it('refresh replaces previous state', async () => {
    usePreviewStateStore.setState({ activeDeviceIds: new Set(['old-cam']) })

    vi.mocked(invoke).mockResolvedValue(['new-cam'])

    await usePreviewStateStore.getState().refresh()

    const { activeDeviceIds } = usePreviewStateStore.getState()
    expect(activeDeviceIds.has('old-cam')).toBe(false)
    expect(activeDeviceIds.has('new-cam')).toBe(true)
  })

  it('refresh with empty result clears active devices', async () => {
    usePreviewStateStore.setState({ activeDeviceIds: new Set(['cam-1']) })

    vi.mocked(invoke).mockResolvedValue([])

    await usePreviewStateStore.getState().refresh()

    expect(usePreviewStateStore.getState().activeDeviceIds.size).toBe(0)
  })
})
