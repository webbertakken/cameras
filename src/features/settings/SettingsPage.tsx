import { GpuAdapterSelector } from './GpuAdapterSelector'
import './SettingsPage.css'

export function SettingsPage() {
  return (
    <main className="settings-page">
      <h1 className="settings-page__heading">App Settings</h1>
      <section className="settings-page__section">
        <GpuAdapterSelector />
      </section>
    </main>
  )
}
