import { useContext } from 'react'
import { ThemeContext } from './ThemeContext'

/** Returns the current theme from ThemeProvider context. */
export function useTheme() {
  const ctx = useContext(ThemeContext)
  if (!ctx) throw new Error('useTheme must be used within a ThemeProvider')
  return ctx
}
