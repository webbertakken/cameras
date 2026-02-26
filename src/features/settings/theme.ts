export type Theme = 'light' | 'dark'

const DARK_QUERY = '(prefers-color-scheme: dark)'

/** Reads the current OS theme preference. */
export function getSystemTheme(): Theme {
  return window.matchMedia(DARK_QUERY).matches ? 'dark' : 'light'
}

/** Watches for OS theme changes and invokes the callback. Returns an unsubscribe function. */
export function watchThemeChange(callback: (theme: Theme) => void): () => void {
  const mql = window.matchMedia(DARK_QUERY)
  const handler = (e: MediaQueryListEvent) => callback(e.matches ? 'dark' : 'light')

  mql.addEventListener('change', handler)
  return () => mql.removeEventListener('change', handler)
}
