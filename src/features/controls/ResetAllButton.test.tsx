import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { useToastStore } from '../notifications/useToast'
import { ResetAllButton } from './ResetAllButton'

vi.mock('./api', () => ({
  getCameraControls: vi.fn(),
  setCameraControl: vi.fn(),
  resetCameraControl: vi.fn(),
  resetAllToDefaults: vi.fn(),
  getSavedSettings: vi.fn(),
}))

const { resetAllToDefaults } = await import('./api')
const mockResetAll = vi.mocked(resetAllToDefaults)

const defaultProps = {
  cameraId: 'cam-1',
  cameraName: 'Logitech C920',
  onReset: vi.fn(),
}

describe('ResetAllButton', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    useToastStore.setState({ toasts: [] })
  })

  it('renders "Reset all to defaults" button', () => {
    render(<ResetAllButton {...defaultProps} />)
    expect(screen.getByRole('button', { name: /reset all to defaults/i })).toBeInTheDocument()
  })

  it('opens confirmation modal on click', async () => {
    const user = userEvent.setup()
    render(<ResetAllButton {...defaultProps} />)
    await user.click(screen.getByRole('button', { name: /reset all to defaults/i }))
    expect(screen.getByRole('dialog')).toBeInTheDocument()
  })

  it('modal has correct title and message', async () => {
    const user = userEvent.setup()
    render(<ResetAllButton {...defaultProps} />)
    await user.click(screen.getByRole('button', { name: /reset all to defaults/i }))
    expect(screen.getByText('Reset to defaults?')).toBeInTheDocument()
    expect(
      screen.getByText(/all controls for Logitech C920 will be reset to their hardware defaults/i),
    ).toBeInTheDocument()
  })

  it('calls resetAllToDefaults IPC on confirm', async () => {
    const user = userEvent.setup()
    const results = [{ controlId: 'brightness', value: 128 }]
    mockResetAll.mockResolvedValueOnce(results)
    render(<ResetAllButton {...defaultProps} />)

    await user.click(screen.getByRole('button', { name: /reset all to defaults/i }))
    await user.click(screen.getByRole('button', { name: 'Reset' }))

    await waitFor(() => {
      expect(mockResetAll).toHaveBeenCalledWith('cam-1')
    })
  })

  it('calls onReset with results after successful IPC', async () => {
    const user = userEvent.setup()
    const results = [{ controlId: 'brightness', value: 128 }]
    mockResetAll.mockResolvedValueOnce(results)
    render(<ResetAllButton {...defaultProps} />)

    await user.click(screen.getByRole('button', { name: /reset all to defaults/i }))
    await user.click(screen.getByRole('button', { name: 'Reset' }))

    await waitFor(() => {
      expect(defaultProps.onReset).toHaveBeenCalledWith(results)
    })
  })

  it('shows success toast after reset', async () => {
    const user = userEvent.setup()
    const results = [{ controlId: 'brightness', value: 128 }]
    mockResetAll.mockResolvedValueOnce(results)
    render(<ResetAllButton {...defaultProps} />)

    await user.click(screen.getByRole('button', { name: /reset all to defaults/i }))
    await user.click(screen.getByRole('button', { name: 'Reset' }))

    await waitFor(() => {
      const toasts = useToastStore.getState().toasts
      expect(toasts).toHaveLength(1)
      expect(toasts[0]).toMatchObject({ type: 'success' })
    })
  })

  it('closes modal on cancel without calling IPC', async () => {
    const user = userEvent.setup()
    render(<ResetAllButton {...defaultProps} />)

    await user.click(screen.getByRole('button', { name: /reset all to defaults/i }))
    expect(screen.getByRole('dialog')).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: /cancel/i }))
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    expect(mockResetAll).not.toHaveBeenCalled()
  })

  it('shows error toast when IPC fails', async () => {
    const user = userEvent.setup()
    mockResetAll.mockRejectedValueOnce(new Error('Device not found'))
    render(<ResetAllButton {...defaultProps} />)

    await user.click(screen.getByRole('button', { name: /reset all to defaults/i }))
    await user.click(screen.getByRole('button', { name: 'Reset' }))

    await waitFor(() => {
      const toasts = useToastStore.getState().toasts
      expect(toasts).toHaveLength(1)
      expect(toasts[0]).toMatchObject({ type: 'error' })
    })
  })

  it('button is not disabled by default', () => {
    render(<ResetAllButton {...defaultProps} />)
    expect(screen.getByRole('button', { name: /reset all to defaults/i })).not.toBeDisabled()
  })
})
