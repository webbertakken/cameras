import { act } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { useToastStore } from './useToast'

describe('useToastStore', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    useToastStore.setState({ toasts: [] })
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('adds a toast to the queue', () => {
    act(() => {
      useToastStore.getState().addToast('Camera connected', 'success')
    })

    const { toasts } = useToastStore.getState()
    expect(toasts).toHaveLength(1)
    expect(toasts[0]).toMatchObject({ message: 'Camera connected', type: 'success' })
    expect(toasts[0].id).toBeDefined()
  })

  it('removes a toast by id', () => {
    act(() => {
      useToastStore.getState().addToast('Camera connected', 'success')
    })

    const { toasts } = useToastStore.getState()
    const id = toasts[0].id

    act(() => {
      useToastStore.getState().removeToast(id)
    })

    expect(useToastStore.getState().toasts).toHaveLength(0)
  })

  it('auto-removes toast after 4 seconds', () => {
    act(() => {
      useToastStore.getState().addToast('Camera connected', 'success')
    })

    expect(useToastStore.getState().toasts).toHaveLength(1)

    act(() => {
      vi.advanceTimersByTime(4000)
    })

    expect(useToastStore.getState().toasts).toHaveLength(0)
  })

  it('handles multiple toasts', () => {
    act(() => {
      useToastStore.getState().addToast('Camera 1 connected', 'success')
      useToastStore.getState().addToast('Camera 2 disconnected', 'info')
    })

    expect(useToastStore.getState().toasts).toHaveLength(2)
  })

  it('removes only the targeted toast', () => {
    act(() => {
      useToastStore.getState().addToast('First', 'success')
      useToastStore.getState().addToast('Second', 'info')
    })

    const first = useToastStore.getState().toasts[0]

    act(() => {
      useToastStore.getState().removeToast(first.id)
    })

    const remaining = useToastStore.getState().toasts
    expect(remaining).toHaveLength(1)
    expect(remaining[0].message).toBe('Second')
  })
})
