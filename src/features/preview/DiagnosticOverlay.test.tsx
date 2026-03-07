import { describe, expect, it, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { DiagnosticOverlay } from './DiagnosticOverlay.tsx'
import type { CombinedDiagnostics } from './useDiagnostics.ts'

const mockCombined: CombinedDiagnostics = {
  diagnostics: {
    fps: 29.97,
    frameCount: 900,
    dropCount: 3,
    dropRate: 0.33,
    latencyMs: 12.5,
    bandwidthBps: 5_000_000,
    usbBusInfo: null,
  },
  encoding: {
    encoderKind: 'mfSoftware',
    framesEncoded: 500,
    framesDropped: 1,
    avgEncodeMs: 3.2,
    lastEncodeMs: 2.8,
  },
}

const mockCombinedNoEncoding: CombinedDiagnostics = {
  diagnostics: mockCombined.diagnostics,
  encoding: null,
}

const noop = () => {}

describe('DiagnosticOverlay', () => {
  it('is hidden when visible is false', () => {
    render(<DiagnosticOverlay snapshot={mockCombined} visible={false} onToggle={noop} />)
    expect(screen.queryByRole('status')).not.toBeInTheDocument()
  })

  it('shows stats when visible is true', () => {
    render(<DiagnosticOverlay snapshot={mockCombined} visible={true} onToggle={noop} />)
    expect(screen.getByRole('status')).toBeInTheDocument()
  })

  it('calls onToggle when Stats button is clicked', async () => {
    const user = userEvent.setup()
    const onToggle = vi.fn()
    render(<DiagnosticOverlay snapshot={mockCombined} visible={false} onToggle={onToggle} />)

    await user.click(screen.getByRole('button', { name: 'Stats' }))

    expect(onToggle).toHaveBeenCalledOnce()
  })

  it('displays FPS, drops, latency, bandwidth', () => {
    render(<DiagnosticOverlay snapshot={mockCombined} visible={true} onToggle={noop} />)

    expect(screen.getByText('30.0')).toBeInTheDocument()
    expect(screen.getByText('3')).toBeInTheDocument()
    expect(screen.getByText('0.3%')).toBeInTheDocument()
    expect(screen.getByText('12.5 ms')).toBeInTheDocument()
    expect(screen.getByText('5.0 MB/s')).toBeInTheDocument()
  })

  it('has semi-transparent background for readability', () => {
    render(<DiagnosticOverlay snapshot={mockCombined} visible={true} onToggle={noop} />)

    const overlay = screen.getByRole('status')
    expect(overlay.className).toContain('diagnostic-overlay')
  })

  it('renders USB bus info when present', () => {
    const snapshotWithUsb: CombinedDiagnostics = {
      ...mockCombined,
      diagnostics: { ...mockCombined.diagnostics, usbBusInfo: 'USB 3.0 Bus 2' },
    }
    render(<DiagnosticOverlay snapshot={snapshotWithUsb} visible={true} onToggle={noop} />)

    expect(screen.getByText('USB 3.0 Bus 2')).toBeInTheDocument()
  })

  it('omits USB bus info row when null', () => {
    render(<DiagnosticOverlay snapshot={mockCombined} visible={true} onToggle={noop} />)
    expect(screen.queryByText('USB bus')).not.toBeInTheDocument()
  })

  it('toggle button reflects aria-pressed from visible prop', () => {
    const { rerender } = render(
      <DiagnosticOverlay snapshot={mockCombined} visible={false} onToggle={noop} />,
    )
    const button = screen.getByRole('button', { name: 'Stats' })
    expect(button).toHaveAttribute('aria-pressed', 'false')

    rerender(<DiagnosticOverlay snapshot={mockCombined} visible={true} onToggle={noop} />)
    expect(button).toHaveAttribute('aria-pressed', 'true')
  })

  it('renders encoding stats when available', () => {
    render(<DiagnosticOverlay snapshot={mockCombined} visible={true} onToggle={noop} />)

    expect(screen.getByText('mfSoftware')).toBeInTheDocument()
    expect(screen.getByText('3.2 ms')).toBeInTheDocument()
    expect(screen.getByText('2.8 ms')).toBeInTheDocument()
    expect(screen.getByText('500')).toBeInTheDocument()
  })

  it('hides encoding section when encoding is null', () => {
    render(<DiagnosticOverlay snapshot={mockCombinedNoEncoding} visible={true} onToggle={noop} />)

    expect(screen.queryByText('Encoder')).not.toBeInTheDocument()
    expect(screen.queryByText('Encode avg')).not.toBeInTheDocument()
  })

  it('colour-codes mfHardware encoder kind green', () => {
    const snapshot: CombinedDiagnostics = {
      ...mockCombined,
      encoding: {
        ...mockCombined.encoding,
        encoderKind: 'mfHardware',
      } as CombinedDiagnostics['encoding'],
    }
    render(<DiagnosticOverlay snapshot={snapshot} visible={true} onToggle={noop} />)

    const encoderValue = screen.getByText('mfHardware')
    expect(encoderValue.className).toContain('encoder--hardware')
  })

  it('colour-codes mfSoftware encoder kind amber', () => {
    render(<DiagnosticOverlay snapshot={mockCombined} visible={true} onToggle={noop} />)

    const encoderValue = screen.getByText('mfSoftware')
    expect(encoderValue.className).toContain('encoder--software')
  })

  it('colour-codes cpuFallback encoder kind red', () => {
    const snapshot: CombinedDiagnostics = {
      ...mockCombined,
      encoding: {
        ...mockCombined.encoding,
        encoderKind: 'cpuFallback',
      } as CombinedDiagnostics['encoding'],
    }
    render(<DiagnosticOverlay snapshot={snapshot} visible={true} onToggle={noop} />)

    const encoderValue = screen.getByText('cpuFallback')
    expect(encoderValue.className).toContain('encoder--cpu')
  })
})
