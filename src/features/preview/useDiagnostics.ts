import { useEffect, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'

export interface DiagnosticSnapshot {
  fps: number
  frameCount: number
  dropCount: number
  dropRate: number
  latencyMs: number
  bandwidthBps: number
  usbBusInfo: string | null
}

export interface EncodingSnapshot {
  encoderKind: string
  framesEncoded: number
  framesDropped: number
  avgEncodeMs: number
  lastEncodeMs: number
}

export interface CombinedDiagnostics {
  diagnostics: DiagnosticSnapshot
  encoding: EncodingSnapshot | null
}

/** Polls diagnostic and encoding stats at 1fps (1000ms interval). */
export function useDiagnostics(
  deviceId: string | null,
  enabled: boolean,
): CombinedDiagnostics | null {
  const [state, setState] = useState<{
    key: string
    snapshot: CombinedDiagnostics | null
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
      const [diagResult, encResult] = await Promise.allSettled([
        invoke<DiagnosticSnapshot>('get_diagnostics', { deviceId }),
        invoke<EncodingSnapshot>('get_encoding_stats', { deviceId }),
      ])

      if (diagResult.status === 'rejected') return

      const diagnostics = diagResult.value
      const encoding = encResult.status === 'fulfilled' ? encResult.value : null

      setState((prev) => ({ ...prev, snapshot: { diagnostics, encoding } }))
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
