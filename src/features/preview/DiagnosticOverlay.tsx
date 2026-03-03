import type { CombinedDiagnostics } from './useDiagnostics.ts'
import './DiagnosticOverlay.css'

interface DiagnosticOverlayProps {
  snapshot: CombinedDiagnostics | null
  visible: boolean
  onToggle: () => void
}

function formatBandwidth(bps: number): string {
  if (bps >= 1_000_000) return `${(bps / 1_000_000).toFixed(1)} MB/s`
  if (bps >= 1_000) return `${(bps / 1_000).toFixed(1)} KB/s`
  return `${bps} B/s`
}

/** Maps encoder kind to a CSS class for colour coding. */
function encoderClass(kind: string): string {
  switch (kind) {
    case 'mfHardware':
      return 'encoder--hardware'
    case 'mfSoftware':
      return 'encoder--software'
    case 'cpuFallback':
      return 'encoder--cpu'
    default:
      return ''
  }
}

/** Toggleable diagnostic stats overlay for the preview canvas. */
export function DiagnosticOverlay({ snapshot, visible, onToggle }: DiagnosticOverlayProps) {
  return (
    <>
      <button
        className="diagnostic-overlay__toggle"
        type="button"
        aria-pressed={visible}
        onClick={onToggle}
        title="Toggle diagnostics"
      >
        Stats
      </button>
      {visible && snapshot && (
        <div className="diagnostic-overlay" role="status" aria-label="Diagnostic statistics">
          <dl className="diagnostic-overlay__grid">
            <dt>FPS</dt>
            <dd>{snapshot.diagnostics.fps.toFixed(1)}</dd>
            <dt>Drops</dt>
            <dd>{snapshot.diagnostics.dropCount}</dd>
            <dt>Drop rate</dt>
            <dd>{snapshot.diagnostics.dropRate.toFixed(1)}%</dd>
            <dt>Latency</dt>
            <dd>{snapshot.diagnostics.latencyMs.toFixed(1)} ms</dd>
            <dt>Bandwidth</dt>
            <dd>{formatBandwidth(snapshot.diagnostics.bandwidthBps)}</dd>
            {snapshot.diagnostics.usbBusInfo && (
              <>
                <dt>USB bus</dt>
                <dd className="diagnostic-overlay__truncate">{snapshot.diagnostics.usbBusInfo}</dd>
              </>
            )}
          </dl>
          {snapshot.encoding && (
            <dl className="diagnostic-overlay__grid diagnostic-overlay__encoding">
              <dt>Encoder</dt>
              <dd className={encoderClass(snapshot.encoding.encoderKind)}>
                {snapshot.encoding.encoderKind}
              </dd>
              <dt>Encode avg</dt>
              <dd>{snapshot.encoding.avgEncodeMs.toFixed(1)} ms</dd>
              <dt>Encode last</dt>
              <dd>{snapshot.encoding.lastEncodeMs.toFixed(1)} ms</dd>
              <dt>Encoded</dt>
              <dd>{snapshot.encoding.framesEncoded}</dd>
              <dt>Encode drops</dt>
              <dd>{snapshot.encoding.framesDropped}</dd>
            </dl>
          )}
        </div>
      )}
    </>
  )
}
