import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { Toast } from './Toast'

describe('Toast', () => {
  it('renders toast with message and type', () => {
    render(<Toast id="1" message="Logitech C920 connected" type="success" onDismiss={vi.fn()} />)

    expect(screen.getByText('Logitech C920 connected')).toBeInTheDocument()
  })

  it('has correct aria-live attribute', () => {
    render(<Toast id="1" message="Camera connected" type="success" onDismiss={vi.fn()} />)

    expect(screen.getByRole('status')).toHaveAttribute('aria-live', 'polite')
  })

  it('renders dismiss button with accessible label', () => {
    render(<Toast id="1" message="Camera connected" type="success" onDismiss={vi.fn()} />)

    expect(screen.getByRole('button', { name: 'Dismiss notification' })).toBeInTheDocument()
  })

  it('calls onDismiss when dismiss button is clicked', async () => {
    const onDismiss = vi.fn()
    render(<Toast id="t1" message="Camera connected" type="success" onDismiss={onDismiss} />)

    await userEvent.click(screen.getByRole('button', { name: 'Dismiss notification' }))

    expect(onDismiss).toHaveBeenCalledWith('t1')
  })

  it('applies success variant class', () => {
    render(<Toast id="1" message="Connected" type="success" onDismiss={vi.fn()} />)

    expect(screen.getByRole('status')).toHaveClass('toast--success')
  })

  it('applies info variant class', () => {
    render(<Toast id="1" message="Disconnected" type="info" onDismiss={vi.fn()} />)

    expect(screen.getByRole('status')).toHaveClass('toast--info')
  })
})
