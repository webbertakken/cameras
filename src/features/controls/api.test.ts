import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { ControlDescriptor } from '../../types/camera'
import { getCameraControls, resetCameraControl, setCameraControl } from './api'

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))

const { invoke } = await import('@tauri-apps/api/core')
const mockInvoke = vi.mocked(invoke)

const brightness: ControlDescriptor = {
  id: 'brightness',
  name: 'Brightness',
  controlType: 'slider',
  group: 'image',
  min: 0,
  max: 255,
  step: 1,
  default: 128,
  current: 128,
  flags: { supportsAuto: false, isAutoEnabled: false, isReadOnly: false },
  supported: true,
}

describe('controls API', () => {
  beforeEach(() => {
    mockInvoke.mockReset()
  })

  it('fetches controls for a given device id', async () => {
    mockInvoke.mockResolvedValueOnce([brightness])
    const result = await getCameraControls('cam-1')
    expect(mockInvoke).toHaveBeenCalledWith('get_camera_controls', { deviceId: 'cam-1' })
    expect(result).toEqual([brightness])
  })

  it('calls set_camera_control with correct IPC args', async () => {
    mockInvoke.mockResolvedValueOnce(undefined)
    await setCameraControl('cam-1', 'brightness', 200)
    expect(mockInvoke).toHaveBeenCalledWith('set_camera_control', {
      deviceId: 'cam-1',
      controlId: 'brightness',
      value: 200,
    })
  })

  it('calls reset_camera_control and returns default value', async () => {
    mockInvoke.mockResolvedValueOnce(128)
    const result = await resetCameraControl('cam-1', 'brightness')
    expect(mockInvoke).toHaveBeenCalledWith('reset_camera_control', {
      deviceId: 'cam-1',
      controlId: 'brightness',
    })
    expect(result).toBe(128)
  })

  it('propagates backend errors as rejected promises', async () => {
    mockInvoke.mockRejectedValueOnce(new Error('Device not found'))
    await expect(getCameraControls('nonexistent')).rejects.toThrow('Device not found')
  })
})
