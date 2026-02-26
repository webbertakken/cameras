import { describe, expect, it } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { DiagnosticOverlay } from './DiagnosticOverlay.tsx'
import type { DiagnosticSnapshot } from './useDiagnostics.ts'

const mockSnapshot: DiagnosticSnapshot = {
  fps: 29.97,
  frameCount: 900,
  dropCount: 3,
  dropRate: 0.33,
  latencyMs: 12.5,
  bandwidthBps: 5_000_000,
}

describe('DiagnosticOverlay', () => {
  it('is hidden by default', () => {
    render(<DiagnosticOverlay snapshot={mockSnapshot} />)
    expect(screen.queryByRole('status')).not.toBeInTheDocument()
  })

  it('shows stats when toggled on', async () => {
    const user = userEvent.setup()
    render(<DiagnosticOverlay snapshot={mockSnapshot} />)

    await user.click(screen.getByRole('button', { name: 'Stats' }))

    expect(screen.getByRole('status')).toBeInTheDocument()
  })

  it('displays FPS, drops, latency, bandwidth', async () => {
    const user = userEvent.setup()
    render(<DiagnosticOverlay snapshot={mockSnapshot} />)

    await user.click(screen.getByRole('button', { name: 'Stats' }))

    expect(screen.getByText('30.0')).toBeInTheDocument()
    expect(screen.getByText('3')).toBeInTheDocument()
    expect(screen.getByText('0.3%')).toBeInTheDocument()
    expect(screen.getByText('12.5 ms')).toBeInTheDocument()
    expect(screen.getByText('5.0 MB/s')).toBeInTheDocument()
  })

  it('has semi-transparent background for readability', async () => {
    const user = userEvent.setup()
    render(<DiagnosticOverlay snapshot={mockSnapshot} />)

    await user.click(screen.getByRole('button', { name: 'Stats' }))

    const overlay = screen.getByRole('status')
    expect(overlay.className).toContain('diagnostic-overlay')
  })

  it('toggle button has correct aria-pressed state', async () => {
    const user = userEvent.setup()
    render(<DiagnosticOverlay snapshot={mockSnapshot} />)

    const button = screen.getByRole('button', { name: 'Stats' })
    expect(button).toHaveAttribute('aria-pressed', 'false')

    await user.click(button)
    expect(button).toHaveAttribute('aria-pressed', 'true')

    await user.click(button)
    expect(button).toHaveAttribute('aria-pressed', 'false')
  })
})
