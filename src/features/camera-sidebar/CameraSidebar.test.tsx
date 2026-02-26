import { fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, it } from 'vitest'
import type { CameraDevice } from '../../types/camera'
import { CameraSidebar } from './CameraSidebar'
import { useCameraStore } from './store'

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
    useCameraStore.setState({ cameras: [], selectedId: null })
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
})
