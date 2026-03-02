import { useCallback, useEffect, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useToastStore } from '../notifications/useToast'

interface UsePreviewResult {
  frameSrc: string | null
  isActive: boolean
  error: string | null
  /** Start the display loop — reads frames from the already-running backend session. */
  start: () => void
  /** Stop the display loop. Does NOT stop the backend capture session. */
  stop: () => void
}

/** Maximum consecutive get_frame failures before giving up (~2.5s at 60fps). */
const MAX_CONSECUTIVE_FAILURES = 150

/** Grace period after starting the display loop during which failures are
 *  ignored (ms). Gives the backend time to produce its first frames. */
const STARTUP_GRACE_MS = 5_000

/**
 * Hook managing the frame display loop for a single camera.
 *
 * Capture sessions are started by the backend at startup and on hotplug.
 * This hook only drives the frame-fetch rAF loop — it does NOT call
 * `start_preview` or `stop_preview` IPC commands.
 */
export function usePreview(deviceId: string | null): UsePreviewResult {
  const [frameSrc, setFrameSrc] = useState<string | null>(null)
  const [isActive, setIsActive] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const runningRef = useRef(false)
  const rafIdRef = useRef<number | null>(null)
  const prevBlobUrlRef = useRef<string | null>(null)
  const failureCountRef = useRef(0)
  const startTimeRef = useRef(0)

  const cancelLoop = useCallback(() => {
    runningRef.current = false
    if (rafIdRef.current !== null) {
      cancelAnimationFrame(rafIdRef.current)
      rafIdRef.current = null
    }
  }, [])

  const stop = useCallback(() => {
    cancelLoop()
    if (prevBlobUrlRef.current) {
      URL.revokeObjectURL(prevBlobUrlRef.current)
      prevBlobUrlRef.current = null
    }
    setIsActive(false)
    setFrameSrc(null)
  }, [cancelLoop])

  const start = useCallback(() => {
    if (!deviceId) return

    // Cancel any existing loop before starting a new one
    cancelLoop()
    if (prevBlobUrlRef.current) {
      URL.revokeObjectURL(prevBlobUrlRef.current)
      prevBlobUrlRef.current = null
    }
    setError(null)

    setIsActive(true)
    runningRef.current = true
    failureCountRef.current = 0
    startTimeRef.current = Date.now()

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
        const elapsed = Date.now() - startTimeRef.current
        if (elapsed < STARTUP_GRACE_MS) {
          // During startup grace period, reset the counter so the camera
          // has time to initialise its capture graph before we give up.
          failureCountRef.current = 0
        } else {
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
      }
      if (runningRef.current) {
        rafIdRef.current = requestAnimationFrame(() => void fetchFrame())
      }
    }

    rafIdRef.current = requestAnimationFrame(() => void fetchFrame())
  }, [deviceId, cancelLoop])

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
