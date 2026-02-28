import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { Root } from './Root'
import { ThemeProvider } from './features/settings/ThemeProvider'
import './styles/tokens.css'
import './styles/base.css'

const root = document.getElementById('root')
if (!root) throw new Error('Root element not found')

createRoot(root).render(
  <StrictMode>
    <ThemeProvider>
      <Root />
    </ThemeProvider>
  </StrictMode>,
)
