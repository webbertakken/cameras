import type { ControlDescriptor } from '../../types/camera'
import './ControlToggle.css'

interface ControlToggleProps {
  descriptor: ControlDescriptor
  value: number
  onChange: (value: number) => void
  disabled?: boolean
  cameraName?: string
}

export function ControlToggle({
  descriptor,
  value,
  onChange,
  disabled = false,
  cameraName,
}: ControlToggleProps) {
  const isOn = value !== 0
  const tooltip = disabled && cameraName ? `Not supported by ${cameraName}` : undefined

  function toggle() {
    if (disabled) return
    onChange(isOn ? 0 : 1)
  }

  return (
    <div className={`control-toggle${disabled ? ' control-toggle--disabled' : ''}`} title={tooltip}>
      <span className="control-toggle__label">{descriptor.name}</span>
      <button
        type="button"
        role="switch"
        className={`control-toggle__switch${isOn ? ' control-toggle__switch--on' : ''}`}
        aria-checked={isOn}
        aria-label={descriptor.name}
        disabled={disabled}
        onClick={toggle}
      >
        <span className="control-toggle__thumb" />
      </button>
    </div>
  )
}
