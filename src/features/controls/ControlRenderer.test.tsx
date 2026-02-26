import { render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { ControlDescriptor } from '../../types/camera'
import { ControlRenderer } from './ControlRenderer'

const slider: ControlDescriptor = {
  id: 'brightness',
  name: 'Brightness',
  controlType: 'slider',
  group: 'image',
  min: 0,
  max: 255,
  step: 1,
  default: 128,
  current: 128,
  flags: { supportsAuto: false, isAutoEnabled: false, isReadOnly: false },
  supported: true,
}

const toggle: ControlDescriptor = {
  id: 'color_enable',
  name: 'Colour Enable',
  controlType: 'toggle',
  group: 'advanced',
  min: 0,
  max: 1,
  step: 1,
  default: 1,
  current: 1,
  flags: { supportsAuto: false, isAutoEnabled: false, isReadOnly: false },
  supported: true,
}

const select: ControlDescriptor = {
  id: 'backlight_compensation',
  name: 'Backlight Compensation',
  controlType: 'select',
  group: 'exposure',
  min: 0,
  max: 3,
  step: 1,
  default: 1,
  current: 1,
  flags: { supportsAuto: false, isAutoEnabled: false, isReadOnly: false },
  supported: true,
}

const unsupported: ControlDescriptor = {
  ...slider,
  id: 'pan',
  name: 'Pan',
  supported: false,
}

describe('ControlRenderer', () => {
  it('renders ControlSlider for controlType "slider"', () => {
    render(
      <ControlRenderer
        descriptor={slider}
        value={128}
        cameraName="Test Cam"
        onChange={vi.fn()}
        onReset={vi.fn()}
      />,
    )
    expect(screen.getByRole('slider')).toBeInTheDocument()
  })

  it('renders ControlToggle for controlType "toggle"', () => {
    render(
      <ControlRenderer
        descriptor={toggle}
        value={1}
        cameraName="Test Cam"
        onChange={vi.fn()}
        onReset={vi.fn()}
      />,
    )
    expect(screen.getByRole('switch')).toBeInTheDocument()
  })

  it('renders ControlSelect for controlType "select"', () => {
    render(
      <ControlRenderer
        descriptor={select}
        value={1}
        cameraName="Test Cam"
        onChange={vi.fn()}
        onReset={vi.fn()}
      />,
    )
    expect(screen.getByRole('combobox')).toBeInTheDocument()
  })

  it('passes descriptor props through correctly', () => {
    render(
      <ControlRenderer
        descriptor={slider}
        value={200}
        cameraName="Test Cam"
        onChange={vi.fn()}
        onReset={vi.fn()}
      />,
    )
    expect(screen.getByText('Brightness')).toBeInTheDocument()
    expect(screen.getByText('200')).toBeInTheDocument()
  })

  it('renders disabled state for unsupported controls', () => {
    render(
      <ControlRenderer
        descriptor={unsupported}
        value={0}
        cameraName="Logitech C920"
        onChange={vi.fn()}
        onReset={vi.fn()}
      />,
    )
    expect(screen.getByRole('slider')).toBeDisabled()
  })

  it('shows "Not supported by [Camera Name]" tooltip for unsupported', () => {
    render(
      <ControlRenderer
        descriptor={unsupported}
        value={0}
        cameraName="Logitech C920"
        onChange={vi.fn()}
        onReset={vi.fn()}
      />,
    )
    const container = screen.getByRole('slider').closest('.control-slider')
    expect(container).toHaveAttribute('title', 'Not supported by Logitech C920')
  })
})
