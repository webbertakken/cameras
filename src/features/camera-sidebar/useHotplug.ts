import { useEffect } from 'react'
import type { HotplugEvent } from '../../types/camera'
import { onCameraHotplug } from './api'
import { useCameraStore } from './store'

/** Subscribes to camera hot-plug events and updates the store. */
export function useHotplug() {
  useEffect(() => {
    let unlisten: (() => void) | undefined

    onCameraHotplug((event: HotplugEvent) => {
      if (event.type === 'connected') {
        useCameraStore.getState().addCamera({
          id: event.id,
          name: event.name ?? 'Unknown Camera',
          devicePath: event.devicePath ?? '',
          isConnected: event.isConnected ?? true,
        })
      } else if (event.type === 'disconnected') {
        useCameraStore.getState().removeCamera(event.id)
      }
    }).then((fn) => {
      unlisten = fn
    })

    return () => {
      unlisten?.()
    }
  }, [])
}
