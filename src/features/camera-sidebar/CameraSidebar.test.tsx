import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { CameraDevice } from '../../types/camera'
import { CameraSidebar } from './CameraSidebar'
import { usePreviewStateStore } from './preview-state-store'
import { useCameraStore } from './store'
import { useVirtualCameraStore } from './virtual-camera-store'

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue([]),
}))

vi.mock('./virtual-camera-api', () => ({
  startVirtualCamera: vi.fn().mockResolvedValue(undefined),
  stopVirtualCamera: vi.fn().mockResolvedValue(undefined),
  getVirtualCameraStatus: vi.fn().mockResolvedValue(false),
}))

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

describe('CameraSidebar', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    useCameraStore.setState({ cameras: [], selectedId: null })
    usePreviewStateStore.setState({ activeDeviceIds: new Set() })
    useVirtualCameraStore.setState({ activeDevices: new Set() })
  })

  it('renders empty state when no cameras are available', () => {
    render(<CameraSidebar />)
    expect(screen.getByText('No cameras found')).toBeInTheDocument()
  })

  it('renders camera entries when cameras are available', () => {
    useCameraStore.setState({ cameras: [cam1, cam2] })
    render(<CameraSidebar />)
    expect(screen.getByText('Logitech C920')).toBeInTheDocument()
    expect(screen.getByText('Razer Kiyo')).toBeInTheDocument()
  })

  it('selects a camera when clicked', () => {
    useCameraStore.setState({ cameras: [cam1, cam2] })
    render(<CameraSidebar />)
    fireEvent.click(screen.getByText('Logitech C920'))
    expect(useCameraStore.getState().selectedId).toBe('cam-1')
  })

  it('marks selected camera with aria-selected true', () => {
    useCameraStore.setState({ cameras: [cam1, cam2], selectedId: 'cam-1' })
    render(<CameraSidebar />)
    const options = screen.getAllByRole('option')
    expect(options[0]).toHaveAttribute('aria-selected', 'true')
    expect(options[1]).toHaveAttribute('aria-selected', 'false')
  })

  it('allows only one camera to be selected at a time', () => {
    useCameraStore.setState({ cameras: [cam1, cam2] })
    render(<CameraSidebar />)
    fireEvent.click(screen.getByText('Logitech C920'))
    fireEvent.click(screen.getByText('Razer Kiyo'))
    expect(useCameraStore.getState().selectedId).toBe('cam-2')
    const options = screen.getAllByRole('option')
    expect(options[0]).toHaveAttribute('aria-selected', 'false')
    expect(options[1]).toHaveAttribute('aria-selected', 'true')
  })

  it('selects camera via keyboard Enter', async () => {
    useCameraStore.setState({ cameras: [cam1, cam2] })
    render(<CameraSidebar />)
    const option = screen.getAllByRole('option')[0]
    option.focus()
    await userEvent.keyboard('{Enter}')
    expect(useCameraStore.getState().selectedId).toBe('cam-1')
  })

  it('selects camera via keyboard Space', async () => {
    useCameraStore.setState({ cameras: [cam1, cam2] })
    render(<CameraSidebar />)
    const option = screen.getAllByRole('option')[0]
    option.focus()
    await userEvent.keyboard(' ')
    expect(useCameraStore.getState().selectedId).toBe('cam-1')
  })

  it('has accessible sidebar landmark', () => {
    useCameraStore.setState({ cameras: [cam1] })
    render(<CameraSidebar />)
    expect(screen.getByRole('navigation', { name: 'Camera list' })).toBeInTheDocument()
  })

  it('renders virtual camera toggle for each camera', () => {
    useCameraStore.setState({ cameras: [cam1, cam2] })
    render(<CameraSidebar />)
    const toggles = screen.getAllByRole('button', { name: 'Expose as virtual camera' })
    expect(toggles).toHaveLength(2)
  })

  it('shows active state for virtual camera toggle', () => {
    useCameraStore.setState({ cameras: [cam1, cam2] })
    useVirtualCameraStore.setState({ activeDevices: new Set(['cam-1']) })
    render(<CameraSidebar />)
    expect(screen.getByRole('button', { name: 'Stop virtual camera' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Expose as virtual camera' })).toBeInTheDocument()
  })

  it('calls toggle when virtual camera button is clicked', async () => {
    // Make invoke return cam-1 as active preview so the refresh on mount enables the button
    const { invoke } = await import('@tauri-apps/api/core')
    vi.mocked(invoke).mockResolvedValueOnce(['cam-1'])

    useCameraStore.setState({ cameras: [cam1] })
    render(<CameraSidebar />)

    // Wait for the button to become enabled after the useEffect refresh
    const toggle = await screen.findByRole('button', { name: 'Expose as virtual camera' })
    await waitFor(() => expect(toggle).not.toBeDisabled())

    await userEvent.click(toggle)
    // The toggle fires an async store action (fire-and-forget), so wait for it to settle
    const { startVirtualCamera } = await import('./virtual-camera-api')
    await waitFor(() => {
      expect(startVirtualCamera).toHaveBeenCalledWith('cam-1')
    })
  })

  it('disables virtual camera toggle when no preview is active', () => {
    useCameraStore.setState({ cameras: [cam1] })
    usePreviewStateStore.setState({ activeDeviceIds: new Set() })
    render(<CameraSidebar />)
    const toggle = screen.getByRole('button', { name: 'Expose as virtual camera' })
    expect(toggle).toBeDisabled()
  })

  it('enables virtual camera toggle when preview is active', () => {
    useCameraStore.setState({ cameras: [cam1] })
    usePreviewStateStore.setState({ activeDeviceIds: new Set(['cam-1']) })
    render(<CameraSidebar />)
    const toggle = screen.getByRole('button', { name: 'Expose as virtual camera' })
    expect(toggle).not.toBeDisabled()
  })
})
