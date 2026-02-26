import { createContext } from 'react'
import type { Theme } from './theme'

export interface ThemeContextValue {
  theme: Theme
}

export const ThemeContext = createContext<ThemeContextValue | null>(null)
