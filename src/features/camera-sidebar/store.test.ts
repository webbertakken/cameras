import { beforeEach, describe, expect, it } from 'vitest'
import type { CameraDevice } from '../../types/camera'
import { useCameraStore } from './store'

const cam1: CameraDevice = {
  id: 'cam-1',
  name: 'Logitech C920',
  devicePath: '/dev/video0',
  isConnected: true,
}

const cam2: CameraDevice = {
  id: 'cam-2',
  name: 'Razer Kiyo',
  devicePath: '/dev/video1',
  isConnected: true,
}

const cam3: CameraDevice = {
  id: 'cam-3',
  name: 'Elgato Facecam',
  devicePath: '/dev/video2',
  isConnected: true,
}

describe('useCameraStore', () => {
  beforeEach(() => {
    useCameraStore.setState({ cameras: [], selectedId: null })
  })

  it('has empty cameras array and null selectedId initially', () => {
    const state = useCameraStore.getState()
    expect(state.cameras).toEqual([])
    expect(state.selectedId).toBeNull()
  })

  it('sets cameras list via setCameras', () => {
    useCameraStore.getState().setCameras([cam1, cam2])
    expect(useCameraStore.getState().cameras).toEqual([cam1, cam2])
  })

  it('selects a camera via selectCamera', () => {
    useCameraStore.getState().setCameras([cam1, cam2])
    useCameraStore.getState().selectCamera('cam-2')
    expect(useCameraStore.getState().selectedId).toBe('cam-2')
  })

  it('appends a device via addCamera', () => {
    useCameraStore.getState().setCameras([cam1])
    useCameraStore.getState().addCamera(cam2)
    expect(useCameraStore.getState().cameras).toEqual([cam1, cam2])
  })

  it('removes a device by id via removeCamera', () => {
    useCameraStore.getState().setCameras([cam1, cam2])
    useCameraStore.getState().removeCamera('cam-1')
    expect(useCameraStore.getState().cameras).toEqual([cam2])
  })

  it('clears selectedId when the selected camera is removed', () => {
    useCameraStore.getState().setCameras([cam1])
    useCameraStore.getState().selectCamera('cam-1')
    useCameraStore.getState().removeCamera('cam-1')
    expect(useCameraStore.getState().selectedId).toBeNull()
  })

  it('auto-selects the next camera when the active one is removed', () => {
    useCameraStore.getState().setCameras([cam1, cam2, cam3])
    useCameraStore.getState().selectCamera('cam-2')
    useCameraStore.getState().removeCamera('cam-2')
    expect(useCameraStore.getState().selectedId).toBe('cam-3')
  })

  it('auto-selects previous camera when last item is removed', () => {
    useCameraStore.getState().setCameras([cam1, cam2])
    useCameraStore.getState().selectCamera('cam-2')
    useCameraStore.getState().removeCamera('cam-2')
    expect(useCameraStore.getState().selectedId).toBe('cam-1')
  })

  it('returns the selected camera via selectedCamera', () => {
    useCameraStore.getState().setCameras([cam1, cam2])
    useCameraStore.getState().selectCamera('cam-1')
    expect(useCameraStore.getState().selectedCamera()).toEqual(cam1)
  })

  it('returns undefined from selectedCamera when nothing is selected', () => {
    useCameraStore.getState().setCameras([cam1])
    expect(useCameraStore.getState().selectedCamera()).toBeUndefined()
  })
})
