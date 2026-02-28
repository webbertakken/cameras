import { render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(vi.fn()),
}))

vi.mock('./features/controls/api', () => ({
  getCameraControls: vi.fn().mockResolvedValue([]),
  setCameraControl: vi.fn().mockResolvedValue(undefined),
  resetCameraControl: vi.fn().mockResolvedValue(0),
}))

vi.mock('./features/camera-sidebar/api', () => ({
  listCameras: vi.fn().mockResolvedValue([]),
  onCameraHotplug: vi.fn().mockResolvedValue(vi.fn()),
}))

import { Root } from './Root'

describe('Root hash routing', () => {
  let originalHash: string

  beforeEach(() => {
    originalHash = window.location.hash
  })

  afterEach(() => {
    window.location.hash = originalHash
  })

  it('renders SettingsPage when hash is #settings', () => {
    window.location.hash = '#settings'
    render(<Root />)
    expect(screen.getByRole('heading', { name: 'App Settings' })).toBeInTheDocument()
  })

  it('renders main app when hash is empty', () => {
    window.location.hash = ''
    render(<Root />)
    expect(screen.getByRole('navigation', { name: 'Camera list' })).toBeInTheDocument()
  })

  it('renders main app when hash is something else', () => {
    window.location.hash = '#other'
    render(<Root />)
    expect(screen.getByRole('navigation', { name: 'Camera list' })).toBeInTheDocument()
  })
})
