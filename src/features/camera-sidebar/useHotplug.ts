import { useEffect } from 'react'
import type { HotplugEvent } from '../../types/camera'
import { useToastStore } from '../notifications/useToast'
import { onCameraHotplug } from './api'
import { useCameraStore } from './store'

/** Subscribes to camera hot-plug events and updates the store. */
export function useHotplug() {
  useEffect(() => {
    let unlisten: (() => void) | undefined

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
      unlisten = fn
    })

    return () => {
      unlisten?.()
    }
  }, [])
}
