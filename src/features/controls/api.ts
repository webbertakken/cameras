import { invoke } from '@tauri-apps/api/core'
import type { ControlDescriptor } from '../../types/camera'

/** Fetch all supported controls for a camera. */
export async function getCameraControls(deviceId: string): Promise<ControlDescriptor[]> {
  return invoke<ControlDescriptor[]>('get_camera_controls', { deviceId })
}

/** Set a camera control value. */
export async function setCameraControl(
  deviceId: string,
  controlId: string,
  value: number,
): Promise<void> {
  return invoke('set_camera_control', { deviceId, controlId, value })
}

/** Reset a camera control to its hardware default. Returns the default value. */
export async function resetCameraControl(deviceId: string, controlId: string): Promise<number> {
  return invoke<number>('reset_camera_control', { deviceId, controlId })
}
