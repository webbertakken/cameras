import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { getSystemTheme, watchThemeChange } from './theme'

describe('getSystemTheme', () => {
  it('returns "dark" when OS prefers dark scheme', () => {
    window.matchMedia = vi.fn().mockReturnValue({ matches: true })

    expect(getSystemTheme()).toBe('dark')
    expect(window.matchMedia).toHaveBeenCalledWith('(prefers-color-scheme: dark)')
  })

  it('returns "light" when OS prefers light scheme', () => {
    window.matchMedia = vi.fn().mockReturnValue({ matches: false })

    expect(getSystemTheme()).toBe('light')
  })
})

describe('watchThemeChange', () => {
  let addListenerMock: ReturnType<typeof vi.fn>
  let removeListenerMock: ReturnType<typeof vi.fn>

  beforeEach(() => {
    addListenerMock = vi.fn()
    removeListenerMock = vi.fn()
    window.matchMedia = vi.fn().mockReturnValue({
      matches: false,
      addEventListener: addListenerMock,
      removeEventListener: removeListenerMock,
    })
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  it('registers a change listener on the media query', () => {
    const callback = vi.fn()
    watchThemeChange(callback)

    expect(addListenerMock).toHaveBeenCalledWith('change', expect.any(Function))
  })

  it('invokes callback with "dark" when media query matches', () => {
    const callback = vi.fn()
    watchThemeChange(callback)

    const handler = addListenerMock.mock.calls[0][1]
    handler({ matches: true })

    expect(callback).toHaveBeenCalledWith('dark')
  })

  it('invokes callback with "light" when media query does not match', () => {
    const callback = vi.fn()
    watchThemeChange(callback)

    const handler = addListenerMock.mock.calls[0][1]
    handler({ matches: false })

    expect(callback).toHaveBeenCalledWith('light')
  })

  it('returns an unsubscribe function that removes the listener', () => {
    const callback = vi.fn()
    const unsubscribe = watchThemeChange(callback)

    unsubscribe()

    expect(removeListenerMock).toHaveBeenCalledWith('change', expect.any(Function))
  })
})
