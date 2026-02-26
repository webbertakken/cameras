import { useState } from 'react'
import type { DiagnosticSnapshot } from './useDiagnostics.ts'
import './DiagnosticOverlay.css'

interface DiagnosticOverlayProps {
  snapshot: DiagnosticSnapshot | null
}

function formatBandwidth(bps: number): string {
  if (bps >= 1_000_000) return `${(bps / 1_000_000).toFixed(1)} MB/s`
  if (bps >= 1_000) return `${(bps / 1_000).toFixed(1)} KB/s`
  return `${bps} B/s`
}

/** Toggleable diagnostic stats overlay for the preview canvas. */
export function DiagnosticOverlay({ snapshot }: DiagnosticOverlayProps) {
  const [visible, setVisible] = useState(false)

  return (
    <>
      <button
        className="diagnostic-overlay__toggle"
        type="button"
        aria-pressed={visible}
        onClick={() => setVisible((v) => !v)}
        title="Toggle diagnostics"
      >
        Stats
      </button>
      {visible && snapshot && (
        <div className="diagnostic-overlay" role="status" aria-label="Diagnostic statistics">
          <dl className="diagnostic-overlay__grid">
            <dt>FPS</dt>
            <dd>{snapshot.fps.toFixed(1)}</dd>
            <dt>Drops</dt>
            <dd>{snapshot.dropCount}</dd>
            <dt>Drop rate</dt>
            <dd>{snapshot.dropRate.toFixed(1)}%</dd>
            <dt>Latency</dt>
            <dd>{snapshot.latencyMs.toFixed(1)} ms</dd>
            <dt>Bandwidth</dt>
            <dd>{formatBandwidth(snapshot.bandwidthBps)}</dd>
          </dl>
        </div>
      )}
    </>
  )
}
