import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { ControlDescriptor } from '../../types/camera'
import { ControlToggle } from './ControlToggle'

const colorEnable: ControlDescriptor = {
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

const unsupported: ControlDescriptor = {
  ...colorEnable,
  supported: false,
}

describe('ControlToggle', () => {
  const onChange = vi.fn()

  beforeEach(() => {
    onChange.mockReset()
  })

  it('renders as switch with label', () => {
    render(<ControlToggle descriptor={colorEnable} value={1} onChange={onChange} />)
    expect(screen.getByRole('switch')).toBeInTheDocument()
    expect(screen.getByText('Colour Enable')).toBeInTheDocument()
  })

  it('reflects current on state', () => {
    render(<ControlToggle descriptor={colorEnable} value={1} onChange={onChange} />)
    expect(screen.getByRole('switch')).toHaveAttribute('aria-checked', 'true')
  })

  it('reflects current off state', () => {
    render(<ControlToggle descriptor={colorEnable} value={0} onChange={onChange} />)
    expect(screen.getByRole('switch')).toHaveAttribute('aria-checked', 'false')
  })

  it('calls onChange when toggled', async () => {
    const user = userEvent.setup()
    render(<ControlToggle descriptor={colorEnable} value={0} onChange={onChange} />)
    await user.click(screen.getByRole('switch'))
    expect(onChange).toHaveBeenCalledWith(1)
  })

  it('calls onChange with 0 when toggling off', async () => {
    const user = userEvent.setup()
    render(<ControlToggle descriptor={colorEnable} value={1} onChange={onChange} />)
    await user.click(screen.getByRole('switch'))
    expect(onChange).toHaveBeenCalledWith(0)
  })

  it('renders disabled with tooltip when not supported', () => {
    render(
      <ControlToggle
        descriptor={unsupported}
        value={0}
        onChange={onChange}
        disabled
        cameraName="Logitech C920"
      />,
    )
    const toggle = screen.getByRole('switch')
    expect(toggle).toBeDisabled()
    expect(toggle.closest('.control-toggle')).toHaveAttribute(
      'title',
      'Not supported by Logitech C920',
    )
  })

  it('does not fire onChange when disabled', async () => {
    const user = userEvent.setup()
    render(
      <ControlToggle
        descriptor={unsupported}
        value={0}
        onChange={onChange}
        disabled
        cameraName="Logitech C920"
      />,
    )
    await user.click(screen.getByRole('switch'))
    expect(onChange).not.toHaveBeenCalled()
  })

  it('toggles via keyboard Space', async () => {
    const user = userEvent.setup()
    render(<ControlToggle descriptor={colorEnable} value={0} onChange={onChange} />)
    screen.getByRole('switch').focus()
    await user.keyboard(' ')
    expect(onChange).toHaveBeenCalledWith(1)
  })

  it('toggles via keyboard Enter', async () => {
    const user = userEvent.setup()
    render(<ControlToggle descriptor={colorEnable} value={0} onChange={onChange} />)
    screen.getByRole('switch').focus()
    await user.keyboard('{Enter}')
    expect(onChange).toHaveBeenCalledWith(1)
  })
})
