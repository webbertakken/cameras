import { useEffect, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'

/** Polls for sidebar thumbnail frames at 5fps (200ms interval). */
export function useThumbnail(deviceId: string | null): string | null {
  const [state, setState] = useState<{
    deviceId: string | null
    src: string | null
  }>({ deviceId, src: null })
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null)

  // Reset when deviceId changes during render (no effect needed)
  if (state.deviceId !== deviceId) {
    setState({ deviceId, src: null })
  }

  useEffect(() => {
    if (!deviceId) return

    intervalRef.current = setInterval(async () => {
      try {
        const base64 = await invoke<string>('get_thumbnail', { deviceId })
        setState((prev) => ({ ...prev, src: `data:image/jpeg;base64,${base64}` }))
      } catch {
        // Thumbnail not available yet — skip
      }
    }, 200)

    return () => {
      if (intervalRef.current) {
        clearInterval(intervalRef.current)
        intervalRef.current = null
      }
    }
  }, [deviceId])

  return state.src
}
