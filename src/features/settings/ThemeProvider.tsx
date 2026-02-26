import { type ReactNode, useEffect, useState } from 'react'
import { ThemeContext } from './ThemeContext'
import { type Theme, getSystemTheme, watchThemeChange } from './theme'

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [theme, setTheme] = useState<Theme>(getSystemTheme)

  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme)
  }, [theme])

  useEffect(() => {
    return watchThemeChange(setTheme)
  }, [])

  return <ThemeContext.Provider value={{ theme }}>{children}</ThemeContext.Provider>
}
