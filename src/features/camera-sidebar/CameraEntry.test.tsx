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

  it('renders thumbnail image when thumbnailSrc is provided', () => {
    render(
      <CameraEntry
        device={device}
        isSelected={false}
        onSelect={vi.fn()}
        thumbnailSrc="data:image/jpeg;base64,abc"
      />,
    )
    const img = screen.getByRole('img')
    expect(img).toHaveAttribute('src', 'data:image/jpeg;base64,abc')
  })

  it('renders SVG placeholder when thumbnailSrc is null', () => {
    render(
      <CameraEntry device={device} isSelected={false} onSelect={vi.fn()} thumbnailSrc={null} />,
    )
    expect(screen.queryByRole('img')).not.toBeInTheDocument()
    expect(screen.getByTestId('camera-thumbnail').querySelector('svg')).toBeInTheDocument()
  })

  it('renders SVG placeholder when thumbnailSrc is omitted', () => {
    render(<CameraEntry device={device} isSelected={false} onSelect={vi.fn()} />)
    expect(screen.queryByRole('img')).not.toBeInTheDocument()
    expect(screen.getByTestId('camera-thumbnail').querySelector('svg')).toBeInTheDocument()
  })

  it('thumbnail image has correct alt text for accessibility', () => {
    render(
      <CameraEntry
        device={device}
        isSelected={false}
        onSelect={vi.fn()}
        thumbnailSrc="data:image/jpeg;base64,abc"
      />,
    )
    expect(screen.getByRole('img')).toHaveAttribute('alt', 'Logitech C920 preview')
  })

  it('hides SVG icon when thumbnail image is shown', () => {
    render(
      <CameraEntry
        device={device}
        isSelected={false}
        onSelect={vi.fn()}
        thumbnailSrc="data:image/jpeg;base64,abc"
      />,
    )
    expect(screen.getByTestId('camera-thumbnail').querySelector('svg')).not.toBeInTheDocument()
  })

  it('renders virtual camera toggle when onToggleVirtualCamera is provided', () => {
    render(
      <CameraEntry
        device={device}
        isSelected={false}
        onSelect={vi.fn()}
        hasPreview={true}
        onToggleVirtualCamera={vi.fn()}
      />,
    )
    expect(screen.getByRole('button', { name: 'Expose as virtual camera' })).toBeInTheDocument()
  })

  it('virtual camera button is disabled without active preview', () => {
    render(
      <CameraEntry
        device={device}
        isSelected={false}
        onSelect={vi.fn()}
        hasPreview={false}
        onToggleVirtualCamera={vi.fn()}
      />,
    )
    expect(screen.getByRole('button', { name: 'Expose as virtual camera' })).toBeDisabled()
  })

  it('virtual camera button shows active state', () => {
    render(
      <CameraEntry
        device={device}
        isSelected={false}
        onSelect={vi.fn()}
        hasPreview={true}
        isVirtualCameraActive={true}
        onToggleVirtualCamera={vi.fn()}
      />,
    )
    expect(screen.getByRole('button', { name: 'Stop virtual camera' })).toBeInTheDocument()
  })

  it('virtual camera toggle calls onToggleVirtualCamera with device id', () => {
    const onToggle = vi.fn()
    render(
      <CameraEntry
        device={device}
        isSelected={false}
        onSelect={vi.fn()}
        hasPreview={true}
        onToggleVirtualCamera={onToggle}
      />,
    )
    screen.getByRole('button', { name: 'Expose as virtual camera' }).click()
    expect(onToggle).toHaveBeenCalledWith('cam-1')
  })

  it('does not render virtual camera button when onToggleVirtualCamera is omitted', () => {
    render(<CameraEntry device={device} isSelected={false} onSelect={vi.fn()} />)
    expect(screen.queryByRole('button')).not.toBeInTheDocument()
  })
})
