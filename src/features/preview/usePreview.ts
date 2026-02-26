import { useCallback, useEffect, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'

interface UsePreviewResult {
  frameSrc: string | null
  isActive: boolean
  start: (width: number, height: number, fps: number) => Promise<void>
  stop: () => Promise<void>
}

/** Hook managing the preview lifecycle for a single camera. */
export function usePreview(deviceId: string | null): UsePreviewResult {
  const [frameSrc, setFrameSrc] = useState<string | null>(null)
  const [isActive, setIsActive] = useState(false)
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null)

  const stop = useCallback(async () => {
    if (intervalRef.current) {
      clearInterval(intervalRef.current)
      intervalRef.current = null
    }
    if (deviceId) {
      try {
        await invoke('stop_preview', { deviceId })
      } catch {
        // Idempotent — ignore errors on stop
      }
    }
    setIsActive(false)
    setFrameSrc(null)
  }, [deviceId])

  const start = useCallback(
    async (width: number, height: number, fps: number) => {
      if (!deviceId) return

      await invoke('start_preview', { deviceId, width, height, fps })
      setIsActive(true)

      // Poll for frames at ~30fps
      intervalRef.current = setInterval(async () => {
        try {
          const base64 = await invoke<string>('get_frame', { deviceId })
          setFrameSrc(`data:image/jpeg;base64,${base64}`)
        } catch {
          // Frame not yet available — skip
        }
      }, 33)
    },
    [deviceId],
  )

  // Clean up on unmount or device change
  useEffect(() => {
    return () => {
      if (intervalRef.current) {
        clearInterval(intervalRef.current)
      }
    }
  }, [deviceId])

  return { frameSrc, isActive, start, stop }
}
