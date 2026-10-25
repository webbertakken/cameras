## Context

The app already has all the pieces — they're just not connected:

- **Backend IPC commands** (working, tested):
  - `get_diagnostics(deviceId)` → `DiagnosticSnapshot` (fps, frameCount,
    dropCount, dropRate, latencyMs, bandwidthBps, usbBusInfo)
  - `get_encoding_stats(deviceId)` → `EncodingSnapshot` (encoderKind,
    framesEncoded, framesDropped, avgEncodeMs, lastEncodeMs)
- **Frontend hooks/components** (built but orphaned):
  - `useDiagnostics(deviceId, enabled)` — polls `get_diagnostics` at 1s
  - `DiagnosticOverlay` — renders capture stats in a floating HUD
  - Both exported from `src/features/preview/index.ts` but never imported
    in `App.tsx` or `PreviewCanvas.tsx`

The gap: `useDiagnostics` only fetches capture stats, not encoding stats. The
overlay only renders capture stats. Neither is wired into the UI.

## Goals / non-goals

**Goals:**

- Single overlay showing both capture and encode stats for the selected camera
- Toggle via keyboard shortcut (Ctrl+D) — dev tool, not user-facing
- Polling at 1-2 second intervals (not real-time streaming)
- Immediately useful for diagnosing encode pipeline performance

**Non-goals:**

- Multi-camera simultaneous stats (just the selected camera)
- Persistent stats history, graphs, or charts
- Backend changes (both IPC commands already work)
- Separate settings page or window

## Decisions

### 1. Extend useDiagnostics to also fetch encoding stats

Add a parallel `get_encoding_stats` call inside the existing polling interval.
Merge both responses into a single `CombinedDiagnostics` type. If encoding
stats aren't available (e.g. Canon sessions with no encode worker), the
encoding fields are `null`.

**Why not a separate hook?** Two hooks polling the same device on offset
timers would cause unnecessary render cycles. A single poll that fetches
both is simpler and more efficient.

### 2. Extend DiagnosticOverlay to render encoding rows

Add rows for: encoder kind, avg encode ms, last encode ms, frames
encoded, frames dropped (encode). Use a section divider (thin border) to
visually separate capture stats from encode stats.

Colour-code the encoder kind: green for MF hardware, amber for MF software,
red for CPU fallback. This makes the bottleneck immediately visible.

**Why not a separate component?** The overlay is already a single floating
panel. Adding a second panel creates layout complexity for no benefit.

### 3. Wire into App.tsx with keyboard shortcut

Add a `useEffect` in `App` that listens for `Ctrl+D` (keydown) and toggles
a `showDiagnostics` state. Pass this to `useDiagnostics` as the `enabled`
flag so polling only happens when the overlay is visible.

The overlay renders inside the preview area (absolute positioned), not as a
global panel.

**Why Ctrl+D?** "D" for diagnostics. Not commonly used in Electron/Tauri
apps. Configurable later if it conflicts.

### 4. Keep the "Stats" toggle button as fallback

The existing toggle button in `DiagnosticOverlay` stays for discoverability.
The keyboard shortcut is an additional access method, not a replacement.

## Risks / trade-offs

- **[Double IPC call per poll]** → Two `invoke()` calls per second when
  overlay is visible. Both are lightweight read operations — acceptable
  overhead. Could batch into a single backend command later if needed.
- **[Encoding stats unavailable for Canon]** → Canon sessions don't use the
  encode worker (they deliver native JPEG). Handle gracefully: show "N/A" or
  hide the encoding section.
