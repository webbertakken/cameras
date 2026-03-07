import { invoke } from '@tauri-apps/api/core'

/** Start a virtual camera output for the given device. */
export async function startVirtualCamera(deviceId: string): Promise<void> {
  return invoke('start_virtual_camera', { deviceId })
}

/** Stop the virtual camera output for the given device. */
export async function stopVirtualCamera(deviceId: string): Promise<void> {
  return invoke('stop_virtual_camera', { deviceId })
}

/** Check whether a virtual camera is active for the given device. */
export async function getVirtualCameraStatus(deviceId: string): Promise<boolean> {
  return invoke<boolean>('get_virtual_camera_status', { deviceId })
}
