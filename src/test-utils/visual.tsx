import type { ReactNode } from 'react'
import { render } from 'vitest-browser-react'
import { page } from 'vitest/browser'
import '../styles/tokens.css'
import '../styles/base.css'

/**
 * Render a component with app styles loaded for visual regression testing.
 * Sets the theme to light for consistent baselines.
 */
export async function renderVisual(ui: ReactNode) {
  document.documentElement.setAttribute('data-theme', 'light')
  return render(ui)
}

export { page }
