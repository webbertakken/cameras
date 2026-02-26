import { fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { ControlDescriptor } from '../../types/camera'
import { ControlSlider } from './ControlSlider'

const brightness: ControlDescriptor = {
  id: 'brightness',
  name: 'Brightness',
  controlType: 'slider',
  group: 'image',
  min: 0,
  max: 255,
  step: 1,
  default: 128,
  current: 150,
  flags: { supportsAuto: false, isAutoEnabled: false, isReadOnly: false },
  supported: true,
}

const unsupported: ControlDescriptor = {
  ...brightness,
  id: 'pan',
  name: 'Pan',
  supported: false,
}

describe('ControlSlider', () => {
  const onChange = vi.fn()
  const onReset = vi.fn()

  beforeEach(() => {
    onChange.mockReset()
    onReset.mockReset()
  })

  // --- Rendering ---

  it('renders slider with label, value display, and reset button', () => {
    render(
      <ControlSlider descriptor={brightness} value={150} onChange={onChange} onReset={onReset} />,
    )
    expect(screen.getByText('Brightness')).toBeInTheDocument()
    expect(screen.getByRole('slider')).toBeInTheDocument()
    expect(screen.getByText('150')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /reset/i })).toBeInTheDocument()
  })

  it('sets input range min/max/step from descriptor', () => {
    render(
      <ControlSlider descriptor={brightness} value={150} onChange={onChange} onReset={onReset} />,
    )
    const slider = screen.getByRole('slider')
    expect(slider).toHaveAttribute('min', '0')
    expect(slider).toHaveAttribute('max', '255')
    expect(slider).toHaveAttribute('step', '1')
  })

  it('displays current value in numeric readout', () => {
    render(
      <ControlSlider descriptor={brightness} value={200} onChange={onChange} onReset={onReset} />,
    )
    expect(screen.getByText('200')).toBeInTheDocument()
  })

  // --- Interaction ---

  it('calls onChange with new value when slider input changes', () => {
    render(
      <ControlSlider descriptor={brightness} value={150} onChange={onChange} onReset={onReset} />,
    )
    const slider = screen.getByRole('slider')
    fireEvent.change(slider, { target: { value: '200' } })
    expect(onChange).toHaveBeenCalledWith(200)
  })

  it('allows direct numeric input when readout is clicked', async () => {
    const user = userEvent.setup()
    render(
      <ControlSlider descriptor={brightness} value={150} onChange={onChange} onReset={onReset} />,
    )
    const readout = screen.getByText('150')
    await user.click(readout)
    const input = screen.getByRole('spinbutton')
    expect(input).toBeInTheDocument()
    expect(input).toHaveValue(150)
  })

  it('clamps typed value to min/max range', async () => {
    const user = userEvent.setup()
    render(
      <ControlSlider descriptor={brightness} value={150} onChange={onChange} onReset={onReset} />,
    )
    const readout = screen.getByText('150')
    await user.click(readout)
    const input = screen.getByRole('spinbutton')
    await user.clear(input)
    await user.type(input, '999')
    await user.keyboard('{Enter}')
    // Should clamp to max (255)
    expect(onChange).toHaveBeenCalledWith(255)
  })

  it('calls onReset when reset button is clicked', async () => {
    const user = userEvent.setup()
    render(
      <ControlSlider descriptor={brightness} value={150} onChange={onChange} onReset={onReset} />,
    )
    await user.click(screen.getByRole('button', { name: /reset/i }))
    expect(onReset).toHaveBeenCalledOnce()
  })

  // --- Disabled state ---

  it('renders greyed out when disabled prop is true', () => {
    render(
      <ControlSlider
        descriptor={unsupported}
        value={0}
        onChange={onChange}
        onReset={onReset}
        disabled
        cameraName="Logitech C920"
      />,
    )
    expect(screen.getByRole('slider')).toBeDisabled()
  })

  it('shows tooltip with camera name when disabled', () => {
    render(
      <ControlSlider
        descriptor={unsupported}
        value={0}
        onChange={onChange}
        onReset={onReset}
        disabled
        cameraName="Logitech C920"
      />,
    )
    const container = screen.getByRole('slider').closest('.control-slider')
    expect(container).toHaveAttribute('title', 'Not supported by Logitech C920')
  })

  it('does not fire onChange when disabled', async () => {
    const user = userEvent.setup()
    render(
      <ControlSlider
        descriptor={unsupported}
        value={0}
        onChange={onChange}
        onReset={onReset}
        disabled
        cameraName="Logitech C920"
      />,
    )
    const slider = screen.getByRole('slider')
    await user.click(slider)
    expect(onChange).not.toHaveBeenCalled()
  })

  // --- Error handling ---

  it('displays inline error message when error prop is set', () => {
    render(
      <ControlSlider
        descriptor={brightness}
        value={150}
        onChange={onChange}
        onReset={onReset}
        error="Value rejected by hardware"
      />,
    )
    expect(screen.getByRole('alert')).toHaveTextContent('Value rejected by hardware')
  })

  it('does not display error when error prop is undefined', () => {
    render(
      <ControlSlider descriptor={brightness} value={150} onChange={onChange} onReset={onReset} />,
    )
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
  })

  // --- Accessibility ---

  it('has correct ARIA label for screen readers', () => {
    render(
      <ControlSlider descriptor={brightness} value={128} onChange={onChange} onReset={onReset} />,
    )
    const slider = screen.getByRole('slider')
    expect(slider).toHaveAccessibleName('Brightness')
  })

  it('has aria-valuemin, aria-valuemax, aria-valuenow', () => {
    render(
      <ControlSlider descriptor={brightness} value={128} onChange={onChange} onReset={onReset} />,
    )
    const slider = screen.getByRole('slider')
    expect(slider).toHaveAttribute('aria-valuemin', '0')
    expect(slider).toHaveAttribute('aria-valuemax', '255')
    expect(slider).toHaveAttribute('aria-valuenow', '128')
  })

  it('has visible focus indicator via CSS class', () => {
    render(
      <ControlSlider descriptor={brightness} value={128} onChange={onChange} onReset={onReset} />,
    )
    const slider = screen.getByRole('slider')
    expect(slider.className).toContain('control-slider__input')
  })
})
