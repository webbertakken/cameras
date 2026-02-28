import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { SettingsPage } from './SettingsPage'

describe('SettingsPage', () => {
  it('renders "App Settings" heading', () => {
    render(<SettingsPage />)
    expect(screen.getByRole('heading', { name: 'App Settings' })).toBeInTheDocument()
  })

  it('renders a main landmark', () => {
    render(<SettingsPage />)
    expect(screen.getByRole('main')).toBeInTheDocument()
  })
})
