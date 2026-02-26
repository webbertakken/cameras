import { invoke } from '@tauri-apps/api/core'
import { type UnlistenFn, listen } from '@tauri-apps/api/event'
import type { CameraDevice, HotplugEvent } from '../../types/camera'

/** Fetch the current list of cameras from the Rust backend. */
export async function listCameras(): Promise<CameraDevice[]> {
  return invoke<CameraDevice[]>('list_cameras')
}

/** Subscribe to camera hot-plug events. Returns an unlisten function. */
export async function onCameraHotplug(
  callback: (event: HotplugEvent) => void,
): Promise<UnlistenFn> {
  return listen<HotplugEvent>('camera-hotplug', (event) => {
    callback(event.payload)
  })
}
