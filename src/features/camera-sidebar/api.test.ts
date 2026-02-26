import { type Mock, beforeEach, describe, expect, it, vi } from 'vitest'
import type { CameraDevice } from '../../types/camera'

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(),
}))

import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { listCameras, onCameraHotplug } from './api'

describe('listCameras', () => {
  it('calls invoke with list_cameras', async () => {
    const cameras: CameraDevice[] = [
      { id: 'cam-1', name: 'Webcam', devicePath: '/dev/video0', isConnected: true },
    ]
    ;(invoke as Mock).mockResolvedValue(cameras)

    const result = await listCameras()

    expect(invoke).toHaveBeenCalledWith('list_cameras')
    expect(result).toEqual(cameras)
  })
})

describe('onCameraHotplug', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('calls listen with camera-hotplug event', async () => {
    const unlisten = vi.fn()
    ;(listen as Mock).mockResolvedValue(unlisten)
    const callback = vi.fn()

    await onCameraHotplug(callback)

    expect(listen).toHaveBeenCalledWith('camera-hotplug', expect.any(Function))
  })

  it('returns the unlisten function', async () => {
    const unlisten = vi.fn()
    ;(listen as Mock).mockResolvedValue(unlisten)

    const result = await onCameraHotplug(vi.fn())

    expect(result).toBe(unlisten)
  })

  it('forwards event payload to callback', async () => {
    ;(listen as Mock).mockImplementation((_event: string, handler: (event: unknown) => void) => {
      handler({ payload: { type: 'connected', id: 'cam-1', name: 'Webcam' } })
      return Promise.resolve(vi.fn())
    })
    const callback = vi.fn()

    await onCameraHotplug(callback)

    expect(callback).toHaveBeenCalledWith({ type: 'connected', id: 'cam-1', name: 'Webcam' })
  })
})
