import { useEffect, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'

export interface DiagnosticSnapshot {
  fps: number
  frameCount: number
  dropCount: number
  dropRate: number
  latencyMs: number
  bandwidthBps: number
}

/** Polls diagnostic stats at 1fps (1000ms interval). */
export function useDiagnostics(
  deviceId: string | null,
  enabled: boolean,
): DiagnosticSnapshot | null {
  const [state, setState] = useState<{
    key: string
    snapshot: DiagnosticSnapshot | null
  }>({ key: `${deviceId}:${enabled}`, snapshot: null })
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null)

  // Reset when inputs change during render
  const key = `${deviceId}:${enabled}`
  if (state.key !== key) {
    setState({ key, snapshot: null })
  }

  useEffect(() => {
    if (!deviceId || !enabled) return

    intervalRef.current = setInterval(async () => {
      try {
        const data = await invoke<DiagnosticSnapshot>('get_diagnostics', {
          deviceId,
        })
        setState((prev) => ({ ...prev, snapshot: data }))
      } catch {
        // Diagnostics not available — skip
      }
    }, 1000)

    return () => {
      if (intervalRef.current) {
        clearInterval(intervalRef.current)
        intervalRef.current = null
      }
    }
  }, [deviceId, enabled])

  return state.snapshot
}
