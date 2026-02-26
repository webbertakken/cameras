import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { act } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { ToastContainer } from './ToastContainer'
import { useToastStore } from './useToast'

describe('ToastContainer', () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true })
    useToastStore.setState({ toasts: [] })
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('renders multiple toasts', () => {
    useToastStore.setState({
      toasts: [
        { id: '1', message: 'Camera 1 connected', type: 'success' },
        { id: '2', message: 'Camera 2 disconnected', type: 'info' },
      ],
    })

    render(<ToastContainer />)

    expect(screen.getByText('Camera 1 connected')).toBeInTheDocument()
    expect(screen.getByText('Camera 2 disconnected')).toBeInTheDocument()
  })

  it('removes toast on dismiss click', async () => {
    act(() => {
      useToastStore.getState().addToast('Camera connected', 'success')
    })

    render(<ToastContainer />)
    expect(screen.getByText('Camera connected')).toBeInTheDocument()

    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime })
    await user.click(screen.getByRole('button', { name: 'Dismiss notification' }))

    expect(screen.queryByText('Camera connected')).not.toBeInTheDocument()
  })

  it('renders nothing when there are no toasts', () => {
    const { container } = render(<ToastContainer />)

    expect(container.querySelector('.toast-container')).toBeEmptyDOMElement()
  })
})
