import { useCallback, useEffect, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useToastStore } from '../notifications/useToast'

interface UsePreviewResult {
  frameSrc: string | null
  isActive: boolean
  error: string | null
  start: (width: number, height: number, fps: number) => Promise<void>
  stop: () => Promise<void>
}

/** Maximum consecutive get_frame failures before giving up (~1s at 30fps). */
const MAX_CONSECUTIVE_FAILURES = 30

/** Hook managing the preview lifecycle for a single camera. */
export function usePreview(deviceId: string | null): UsePreviewResult {
  const [frameSrc, setFrameSrc] = useState<string | null>(null)
  const [isActive, setIsActive] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const runningRef = useRef(false)
  const rafIdRef = useRef<number | null>(null)
  const prevBlobUrlRef = useRef<string | null>(null)
  const failureCountRef = useRef(0)

  const cancelLoop = useCallback(() => {
    runningRef.current = false
    if (rafIdRef.current !== null) {
      cancelAnimationFrame(rafIdRef.current)
      rafIdRef.current = null
    }
  }, [])

  const stop = useCallback(async () => {
    cancelLoop()
    if (prevBlobUrlRef.current) {
      URL.revokeObjectURL(prevBlobUrlRef.current)
      prevBlobUrlRef.current = null
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
  }, [deviceId, cancelLoop])

  const start = useCallback(
    async (width: number, height: number, fps: number) => {
      if (!deviceId) return

      // Cancel any existing loop before starting a new one
      cancelLoop()
      if (prevBlobUrlRef.current) {
        URL.revokeObjectURL(prevBlobUrlRef.current)
        prevBlobUrlRef.current = null
      }
      setError(null)

      try {
        await invoke('start_preview', { deviceId, width, height, fps })
      } catch (err: unknown) {
        const message = err instanceof Error ? err.message : String(err)
        setError(message)
        setIsActive(false)
        return
      }

      setIsActive(true)
      runningRef.current = true
      failureCountRef.current = 0

      const fetchFrame = async () => {
        if (!runningRef.current) return
        try {
          const base64 = await invoke<string>('get_frame', { deviceId })
          failureCountRef.current = 0
          const raw = atob(base64)
          const bytes = new Uint8Array(raw.length)
          for (let i = 0; i < raw.length; i++) bytes[i] = raw.charCodeAt(i)
          const blob = new Blob([bytes], { type: 'image/jpeg' })
          const url = URL.createObjectURL(blob)
          if (prevBlobUrlRef.current) {
            URL.revokeObjectURL(prevBlobUrlRef.current)
          }
          prevBlobUrlRef.current = url
          setFrameSrc(url)
        } catch {
          failureCountRef.current++
          if (failureCountRef.current >= MAX_CONSECUTIVE_FAILURES) {
            const msg = 'Preview stopped — camera is not producing frames'
            setError(msg)
            setIsActive(false)
            runningRef.current = false
            useToastStore.getState().addToast(msg, 'error')
            return
          }
        }
        if (runningRef.current) {
          rafIdRef.current = requestAnimationFrame(() => void fetchFrame())
        }
      }

      rafIdRef.current = requestAnimationFrame(() => void fetchFrame())
    },
    [deviceId, cancelLoop],
  )

  // Listen for preview-error events from the Rust backend
  useEffect(() => {
    if (!deviceId) return

    const unlistenPromise = listen<{ deviceId: string; error: string }>(
      'preview-error',
      (event) => {
        if (event.payload.deviceId === deviceId) {
          setError(event.payload.error)
          setFrameSrc(null)
          useToastStore.getState().addToast(event.payload.error, 'error')
        }
      },
    )

    return () => {
      unlistenPromise.then((fn) => fn())
    }
  }, [deviceId])

  // Clean up on unmount or device change
  useEffect(() => {
    return () => {
      cancelLoop()
      if (prevBlobUrlRef.current) {
        URL.revokeObjectURL(prevBlobUrlRef.current)
        prevBlobUrlRef.current = null
      }
    }
  }, [deviceId, cancelLoop])

  return { frameSrc, isActive, error, start, stop }
}
