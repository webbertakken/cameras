import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { act, cleanup, render, screen } from '@testing-library/react'
import { ThemeProvider } from './ThemeProvider'
import { useTheme } from './useTheme'

function ThemeConsumer() {
  const { theme } = useTheme()
  return <span data-testid="theme">{theme}</span>
}

describe('ThemeProvider', () => {
  let addListenerMock: ReturnType<typeof vi.fn>

  beforeEach(() => {
    addListenerMock = vi.fn()
    window.matchMedia = vi.fn().mockReturnValue({
      matches: false,
      addEventListener: addListenerMock,
      removeEventListener: vi.fn(),
    })
    document.documentElement.removeAttribute('data-theme')
  })

  afterEach(() => {
    cleanup()
    vi.restoreAllMocks()
    document.documentElement.removeAttribute('data-theme')
  })

  it('provides the current theme to children', () => {
    render(
      <ThemeProvider>
        <ThemeConsumer />
      </ThemeProvider>,
    )

    expect(screen.getByTestId('theme').textContent).toBe('light')
  })

  it('provides "dark" when OS prefers dark scheme', () => {
    window.matchMedia = vi.fn().mockReturnValue({
      matches: true,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    })

    render(
      <ThemeProvider>
        <ThemeConsumer />
      </ThemeProvider>,
    )

    expect(screen.getByTestId('theme').textContent).toBe('dark')
  })

  it('applies data-theme attribute to the html element', () => {
    render(
      <ThemeProvider>
        <ThemeConsumer />
      </ThemeProvider>,
    )

    expect(document.documentElement.getAttribute('data-theme')).toBe('light')
  })

  it('applies data-theme="dark" when OS prefers dark', () => {
    window.matchMedia = vi.fn().mockReturnValue({
      matches: true,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    })

    render(
      <ThemeProvider>
        <ThemeConsumer />
      </ThemeProvider>,
    )

    expect(document.documentElement.getAttribute('data-theme')).toBe('dark')
  })

  it('updates theme when OS preference changes', () => {
    render(
      <ThemeProvider>
        <ThemeConsumer />
      </ThemeProvider>,
    )

    expect(screen.getByTestId('theme').textContent).toBe('light')

    // Simulate OS theme change
    const handler = addListenerMock.mock.calls[0][1]
    act(() => handler({ matches: true }))

    expect(screen.getByTestId('theme').textContent).toBe('dark')
    expect(document.documentElement.getAttribute('data-theme')).toBe('dark')
  })
})
