import App from './App'
import { SettingsPage } from './features/settings/SettingsPage'

/** Determine which page to render based on the URL hash. */
export function Root() {
  if (window.location.hash === '#settings') {
    return <SettingsPage />
  }
  return <App />
}
