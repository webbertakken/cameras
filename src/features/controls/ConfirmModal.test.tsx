import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { ConfirmModal } from './ConfirmModal'

const defaultProps = {
  open: true,
  title: 'Confirm action',
  message: 'Are you sure you want to proceed?',
  onConfirm: vi.fn(),
  onCancel: vi.fn(),
}

describe('ConfirmModal', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('renders nothing when open is false', () => {
    const { container } = render(<ConfirmModal {...defaultProps} open={false} />)
    expect(container).toBeEmptyDOMElement()
  })

  it('renders modal with title and message when open is true', () => {
    render(<ConfirmModal {...defaultProps} />)
    expect(screen.getByText('Confirm action')).toBeInTheDocument()
    expect(screen.getByText('Are you sure you want to proceed?')).toBeInTheDocument()
  })

  it('calls onConfirm when confirm button clicked', async () => {
    const user = userEvent.setup()
    render(<ConfirmModal {...defaultProps} />)
    await user.click(screen.getByRole('button', { name: /confirm/i }))
    expect(defaultProps.onConfirm).toHaveBeenCalledOnce()
  })

  it('calls onCancel when cancel button clicked', async () => {
    const user = userEvent.setup()
    render(<ConfirmModal {...defaultProps} />)
    await user.click(screen.getByRole('button', { name: /cancel/i }))
    expect(defaultProps.onCancel).toHaveBeenCalledOnce()
  })

  it('calls onCancel on Escape key press', async () => {
    const user = userEvent.setup()
    render(<ConfirmModal {...defaultProps} />)
    await user.keyboard('{Escape}')
    expect(defaultProps.onCancel).toHaveBeenCalledOnce()
  })

  it('calls onCancel when backdrop clicked', async () => {
    const user = userEvent.setup()
    render(<ConfirmModal {...defaultProps} />)
    const overlay = screen.getByRole('dialog').parentElement
    expect(overlay).toBeTruthy()
    await user.click(overlay as HTMLElement)
    expect(defaultProps.onCancel).toHaveBeenCalledOnce()
  })

  it('has role="dialog" and aria-modal="true"', () => {
    render(<ConfirmModal {...defaultProps} />)
    const dialog = screen.getByRole('dialog')
    expect(dialog).toHaveAttribute('aria-modal', 'true')
  })

  it('has aria-labelledby pointing to title', () => {
    render(<ConfirmModal {...defaultProps} />)
    const dialog = screen.getByRole('dialog')
    const labelId = dialog.getAttribute('aria-labelledby')
    expect(labelId).toBeTruthy()
    const title = within(dialog).getByText('Confirm action')
    expect(title.id).toBe(labelId)
  })

  it('uses custom confirmLabel and cancelLabel when provided', () => {
    render(<ConfirmModal {...defaultProps} confirmLabel="Delete" cancelLabel="Keep" />)
    expect(screen.getByRole('button', { name: 'Delete' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Keep' })).toBeInTheDocument()
  })

  it('disables both buttons when confirmDisabled is true', () => {
    render(<ConfirmModal {...defaultProps} confirmDisabled />)
    expect(screen.getByRole('button', { name: /confirm/i })).toBeDisabled()
    expect(screen.getByRole('button', { name: /cancel/i })).toBeDisabled()
  })

  it('does not disable buttons when confirmDisabled is false', () => {
    render(<ConfirmModal {...defaultProps} confirmDisabled={false} />)
    expect(screen.getByRole('button', { name: /confirm/i })).not.toBeDisabled()
    expect(screen.getByRole('button', { name: /cancel/i })).not.toBeDisabled()
  })

  it('does not re-register escape handler when onCancel reference changes', async () => {
    const user = userEvent.setup()
    const onCancel1 = vi.fn()
    const onCancel2 = vi.fn()

    const { rerender } = render(<ConfirmModal {...defaultProps} onCancel={onCancel1} />)
    rerender(<ConfirmModal {...defaultProps} onCancel={onCancel2} />)

    await user.keyboard('{Escape}')
    // Should call the latest onCancel, not the first one
    expect(onCancel2).toHaveBeenCalledOnce()
    expect(onCancel1).not.toHaveBeenCalled()
  })
})
