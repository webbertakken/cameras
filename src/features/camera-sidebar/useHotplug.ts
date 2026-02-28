import { listen } from '@tauri-apps/api/event'
import { useEffect } from 'react'
import type { HotplugEvent, SettingsRestoredPayload } from '../../types/camera'
import { useToastStore } from '../notifications/useToast'
import { onCameraHotplug } from './api'
import { useCameraStore } from './store'

/** Subscribes to camera hot-plug events and settings-restored events. */
export function useHotplug() {
  useEffect(() => {
    let unlistenHotplug: (() => void) | undefined
    let unlistenSettings: (() => void) | undefined

    onCameraHotplug((event: HotplugEvent) => {
      if (event.type === 'connected') {
        const name = event.name ?? 'Unknown Camera'
        useCameraStore.getState().addCamera({
          id: event.id,
          name,
          devicePath: event.devicePath ?? '',
          isConnected: event.isConnected ?? true,
        })
        useToastStore.getState().addToast(`${name} connected`, 'success')
      } else if (event.type === 'disconnected') {
        const camera = useCameraStore.getState().cameras.find((c) => c.id === event.id)
        const name = camera?.name
        useCameraStore.getState().removeCamera(event.id)
        useToastStore
          .getState()
          .addToast(name ? `${name} disconnected` : 'Camera disconnected', 'info')
      }
    }).then((fn) => {
      unlistenHotplug = fn
    })

    listen<SettingsRestoredPayload>('settings-restored', (event) => {
      if (event.payload.controlsApplied > 0) {
        useToastStore
          .getState()
          .addToast(`Settings restored for ${event.payload.cameraName}`, 'success')
      }
    }).then((fn) => {
      unlistenSettings = fn
    })

    return () => {
      unlistenHotplug?.()
      unlistenSettings?.()
    }
  }, [])
}
