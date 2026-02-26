import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { ControlDescriptor } from '../../types/camera'
import { ControlSelect } from './ControlSelect'

const powerline: ControlDescriptor = {
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
  ...powerline,
  supported: false,
}

describe('ControlSelect', () => {
  const onChange = vi.fn()

  beforeEach(() => {
    onChange.mockReset()
  })

  it('renders dropdown with label and current value', () => {
    render(<ControlSelect descriptor={powerline} value={1} onChange={onChange} />)
    expect(screen.getByText('Backlight Compensation')).toBeInTheDocument()
    const select = screen.getByRole('combobox')
    expect(select).toBeInTheDocument()
    expect(select).toHaveValue('1')
  })

  it('lists all options from min to max', () => {
    render(<ControlSelect descriptor={powerline} value={1} onChange={onChange} />)
    const options = screen.getAllByRole('option')
    expect(options).toHaveLength(4) // 0, 1, 2, 3
  })

  it('calls onChange with selected value', async () => {
    const user = userEvent.setup()
    render(<ControlSelect descriptor={powerline} value={1} onChange={onChange} />)
    await user.selectOptions(screen.getByRole('combobox'), '2')
    expect(onChange).toHaveBeenCalledWith(2)
  })

  it('renders disabled with tooltip when not supported', () => {
    render(
      <ControlSelect
        descriptor={unsupported}
        value={0}
        onChange={onChange}
        disabled
        cameraName="Logitech C920"
      />,
    )
    expect(screen.getByRole('combobox')).toBeDisabled()
    expect(screen.getByRole('combobox').closest('.control-select')).toHaveAttribute(
      'title',
      'Not supported by Logitech C920',
    )
  })

  it('has accessible label via aria-label', () => {
    render(<ControlSelect descriptor={powerline} value={1} onChange={onChange} />)
    expect(screen.getByRole('combobox')).toHaveAccessibleName('Backlight Compensation')
  })
})
