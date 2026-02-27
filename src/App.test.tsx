import { render, screen } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { CameraDevice } from './types/camera'
import { useCameraStore } from './features/camera-sidebar/store'

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

import App from './App'

const cam1: CameraDevice = {
  id: 'cam-1',
  name: 'Logitech C920',
  devicePath: '/dev/video0',
  isConnected: true,
}

const cam2: CameraDevice = {
  id: 'cam-2',
  name: 'Razer Kiyo',
  devicePath: '/dev/video1',
  isConnected: true,
}

describe('App', () => {
  beforeEach(() => {
    useCameraStore.setState({ cameras: [], selectedId: null })
  })

  it('renders CameraSidebar', () => {
    render(<App />)
    expect(screen.getByRole('navigation', { name: 'Camera list' })).toBeInTheDocument()
  })

  it('shows placeholder when no camera is selected', () => {
    render(<App />)
    expect(screen.getByText(/select a camera to get started/i)).toBeInTheDocument()
  })

  it('renders ControlsPanel when camera is selected', () => {
    useCameraStore.setState({ cameras: [cam1], selectedId: 'cam-1' })
    render(<App />)
    expect(screen.getByRole('region', { name: 'Camera controls' })).toBeInTheDocument()
  })

  it('renders PreviewCanvas when camera is selected', () => {
    useCameraStore.setState({ cameras: [cam1], selectedId: 'cam-1' })
    render(<App />)
    expect(screen.getByRole('img', { name: 'No preview available' })).toBeInTheDocument()
  })

  it('shows placeholder again when camera is deselected', () => {
    useCameraStore.setState({ cameras: [cam1], selectedId: 'cam-1' })
    const { rerender } = render(<App />)
    expect(screen.queryByText(/select a camera to get started/i)).not.toBeInTheDocument()

    useCameraStore.setState({ selectedId: null })
    rerender(<App />)
    expect(screen.getByText(/select a camera to get started/i)).toBeInTheDocument()
  })

  it('updates main panel when a different camera is selected', () => {
    useCameraStore.setState({ cameras: [cam1, cam2], selectedId: 'cam-1' })
    const { rerender } = render(<App />)
    expect(screen.getByRole('region', { name: 'Camera controls' })).toBeInTheDocument()

    useCameraStore.setState({ selectedId: 'cam-2' })
    rerender(<App />)
    expect(screen.getByRole('region', { name: 'Camera controls' })).toBeInTheDocument()
  })

  it('calls start_preview when camera is selected', async () => {
    const { invoke } = await import('@tauri-apps/api/core')
    const mockInvoke = vi.mocked(invoke)
    mockInvoke.mockResolvedValue(undefined)

    useCameraStore.setState({ cameras: [cam1], selectedId: 'cam-1' })
    render(<App />)

    await vi.waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('start_preview', {
        deviceId: 'cam-1',
        width: 640,
        height: 480,
        fps: 30,
      })
    })
  })
})
