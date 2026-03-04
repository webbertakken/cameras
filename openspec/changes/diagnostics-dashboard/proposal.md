## Why

The encode pipeline's real-time performance is invisible. When frames drop or
the encoder falls back to CPU, developers have no way to see it without
reading logs. We need a heads-up display that shows encode stats alongside
existing capture diagnostics, so bottlenecks are immediately obvious.

## What changes

- **Extend the existing DiagnosticOverlay**: add encoding stats (encoder kind,
  encode time avg/last, frames encoded/dropped) alongside the existing capture
  stats (FPS, latency, bandwidth, drop rate)
- **Wire the overlay into the preview**: the overlay exists but is currently
  orphaned — connect it to `App.tsx` / `PreviewCanvas`
- **Add a keyboard shortcut**: toggle the overlay with a hotkey (e.g. `Ctrl+D`)
  so it's always accessible but never in the way
- **Multi-camera support**: when switching cameras, the overlay polls the
  selected camera's stats — no separate per-camera panels needed

## Capabilities

### Modified capabilities

- `diagnostic-overlay`: Extend the existing `DiagnosticOverlay` and
  `useDiagnostics` to include encoding stats from `get_encoding_stats`

### New capabilities

- None — this extends existing code

## Impact

- **Frontend**: `src/features/preview/useDiagnostics.ts` — also poll
  `get_encoding_stats`, merge into snapshot
- **Frontend**: `src/features/preview/DiagnosticOverlay.tsx` — render encoding
  stats rows
- **Frontend**: `src/App.tsx` — wire `useDiagnostics` + `DiagnosticOverlay`
  into the preview area
- **Backend**: No changes — `get_diagnostics` and `get_encoding_stats` IPC
  commands already exist and work
- **CI**: No changes
