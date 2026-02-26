import type { ControlDescriptor } from '../../types/camera'
import './ControlSelect.css'

interface ControlSelectProps {
  descriptor: ControlDescriptor
  value: number
  onChange: (value: number) => void
  disabled?: boolean
  cameraName?: string
}

export function ControlSelect({
  descriptor,
  value,
  onChange,
  disabled = false,
  cameraName,
}: ControlSelectProps) {
  const min = descriptor.min ?? 0
  const max = descriptor.max ?? 0
  const tooltip = disabled && cameraName ? `Not supported by ${cameraName}` : undefined

  const options: number[] = []
  for (let i = min; i <= max; i += descriptor.step ?? 1) {
    options.push(i)
  }

  return (
    <div className={`control-select${disabled ? ' control-select--disabled' : ''}`} title={tooltip}>
      <label className="control-select__label">{descriptor.name}</label>
      <select
        className="control-select__input"
        role="combobox"
        aria-label={descriptor.name}
        value={String(value)}
        disabled={disabled}
        onChange={(e) => onChange(Number(e.target.value))}
      >
        {options.map((opt) => (
          <option key={opt} value={String(opt)}>
            {opt}
          </option>
        ))}
      </select>
    </div>
  )
}
