# Implementation tasks

OpenSpec: `openspec/changes/diagnostics-dashboard/`

## Task 1: Extend useDiagnostics to fetch encoding stats

**Status**: pending

Modify `src/features/preview/useDiagnostics.ts` to:

- Define `EncodingSnapshot` type: `{ encoderKind: string, framesEncoded:
number, framesDropped: number, avgEncodeMs: number, lastEncodeMs: number }`
- Define `CombinedDiagnostics` type combining `DiagnosticSnapshot` and
  optional `EncodingSnapshot`
- In the polling interval, call both `get_diagnostics` and
  `get_encoding_stats` in parallel (`Promise.allSettled`)
- If `get_encoding_stats` rejects, set encoding fields to `null`
- Update the return type from `DiagnosticSnapshot | null` to
  `CombinedDiagnostics | null`
- Update tests in `useDiagnostics.test.ts`

**TDD approach**: Write tests first for the combined polling behaviour
(both succeed, encoding fails, polling stops).

**Key files**:

- `src/features/preview/useDiagnostics.ts`
- `src/features/preview/useDiagnostics.test.ts`
- `src/features/preview/index.ts` (update export type)

## Task 2: Extend DiagnosticOverlay to render encoding stats

**Status**: pending
**Blocked by**: Task 1

Modify `src/features/preview/DiagnosticOverlay.tsx` to:

- Accept `CombinedDiagnostics` instead of `DiagnosticSnapshot`
- Add a visual separator (thin border) between capture and encode sections
- Render encoding rows when encoding stats are present:
  - Encoder: `{encoderKind}` (colour-coded: green=mfHardware,
    amber=mfSoftware, red=cpuFallback)
  - Encode avg: `{avgEncodeMs}` ms
  - Encode last: `{lastEncodeMs}` ms
  - Encoded: `{framesEncoded}`
  - Encode drops: `{framesDropped}`
- Hide encoding section when encoding is `null`
- Update CSS in `DiagnosticOverlay.css` for colour coding and separator
- Update tests in `DiagnosticOverlay.test.tsx`

**TDD approach**: Write tests for rendering with/without encoding stats,
colour coding per encoder kind.

**Key files**:

- `src/features/preview/DiagnosticOverlay.tsx`
- `src/features/preview/DiagnosticOverlay.css`
- `src/features/preview/DiagnosticOverlay.test.tsx`

## Task 3: Wire overlay into App.tsx with keyboard shortcut

**Status**: pending
**Blocked by**: Task 2

Modify `src/App.tsx` to:

- Add `showDiagnostics` state (default `false`)
- Add `useEffect` for `Ctrl+D` keydown listener to toggle the state
- Call `useDiagnostics(selectedCamera?.id, showDiagnostics)`
- Render `DiagnosticOverlay` inside the preview area (next to `PreviewCanvas`)
  wrapped in a relatively positioned container
- Pass combined diagnostics + showDiagnostics to the overlay

**TDD approach**: Write tests for keyboard shortcut toggling and overlay
rendering in `App.test.tsx`.

**Key files**:

- `src/App.tsx`
- `src/App.test.tsx`
- `src/App.css` (possibly add relative positioning to preview container)

## Task 4: Verify end-to-end

**Status**: pending
**Blocked by**: Task 3

- Start the dev server with a connected camera
- Press Ctrl+D to open the overlay
- Verify all stats appear and update every second
- Verify encoder kind is correct (should match log output)
- Verify overlay hides/shows correctly
- Verify camera switch clears and re-fetches stats
- Check the overlay doesn't interfere with preview or controls
