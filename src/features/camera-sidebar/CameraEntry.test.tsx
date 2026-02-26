import { render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { CameraDevice } from '../../types/camera'
import { CameraEntry } from './CameraEntry'

const device: CameraDevice = {
  id: 'cam-1',
  name: 'Logitech C920',
  devicePath: '/dev/video0',
  isConnected: true,
}

describe('CameraEntry', () => {
  it('renders the camera name', () => {
    render(<CameraEntry device={device} isSelected={false} onSelect={vi.fn()} />)
    expect(screen.getByText('Logitech C920')).toBeInTheDocument()
  })

  it('renders a placeholder thumbnail', () => {
    render(<CameraEntry device={device} isSelected={false} onSelect={vi.fn()} />)
    expect(screen.getByTestId('camera-thumbnail')).toBeInTheDocument()
  })

  it('has correct ARIA role and label', () => {
    render(<CameraEntry device={device} isSelected={false} onSelect={vi.fn()} />)
    const button = screen.getByRole('option')
    expect(button).toHaveAccessibleName('Logitech C920')
  })

  it('applies selected state when isSelected is true', () => {
    render(<CameraEntry device={device} isSelected={true} onSelect={vi.fn()} />)
    expect(screen.getByRole('option')).toHaveAttribute('aria-selected', 'true')
  })

  it('does not apply selected state when isSelected is false', () => {
    render(<CameraEntry device={device} isSelected={false} onSelect={vi.fn()} />)
    expect(screen.getByRole('option')).toHaveAttribute('aria-selected', 'false')
  })

  it('calls onSelect with device id when clicked', () => {
    const onSelect = vi.fn()
    render(<CameraEntry device={device} isSelected={false} onSelect={onSelect} />)
    screen.getByRole('option').click()
    expect(onSelect).toHaveBeenCalledWith('cam-1')
  })
})
