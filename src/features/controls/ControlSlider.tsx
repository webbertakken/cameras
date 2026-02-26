import { useId, useState } from 'react'
import type { ControlDescriptor } from '../../types/camera'
import './ControlSlider.css'

interface ControlSliderProps {
  descriptor: ControlDescriptor
  value: number
  onChange: (value: number) => void
  onReset: () => void
  disabled?: boolean
  cameraName?: string
  error?: string
}

export function ControlSlider({
  descriptor,
  value,
  onChange,
  onReset,
  disabled = false,
  cameraName,
  error,
}: ControlSliderProps) {
  const [isEditing, setIsEditing] = useState(false)
  const [editValue, setEditValue] = useState('')
  const labelId = useId()
  const min = descriptor.min ?? 0
  const max = descriptor.max ?? 255
  const step = descriptor.step ?? 1
  const tooltip = disabled && cameraName ? `Not supported by ${cameraName}` : undefined

  function handleSliderChange(e: React.ChangeEvent<HTMLInputElement>) {
    onChange(Number(e.target.value))
  }

  function startEditing() {
    if (disabled) return
    setEditValue(String(value))
    setIsEditing(true)
  }

  function commitEdit() {
    const parsed = Number(editValue)
    if (!Number.isNaN(parsed)) {
      const clamped = Math.min(max, Math.max(min, parsed))
      onChange(clamped)
    }
    setIsEditing(false)
  }

  function handleEditKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
    if (e.key === 'Enter') {
      commitEdit()
    } else if (e.key === 'Escape') {
      setIsEditing(false)
    }
  }

  return (
    <div className={`control-slider${disabled ? ' control-slider--disabled' : ''}`} title={tooltip}>
      <label id={labelId} className="control-slider__label">
        {descriptor.name}
      </label>
      <div className="control-slider__row">
        <input
          type="range"
          className="control-slider__input"
          role="slider"
          aria-label={descriptor.name}
          aria-labelledby={labelId}
          aria-valuemin={min}
          aria-valuemax={max}
          aria-valuenow={value}
          min={min}
          max={max}
          step={step}
          value={value}
          disabled={disabled}
          onChange={handleSliderChange}
        />
        {isEditing ? (
          <input
            type="number"
            role="spinbutton"
            className="control-slider__edit"
            value={editValue}
            min={min}
            max={max}
            step={step}
            autoFocus
            onChange={(e) => setEditValue(e.target.value)}
            onBlur={commitEdit}
            onKeyDown={handleEditKeyDown}
          />
        ) : (
          <span
            className="control-slider__value"
            onClick={startEditing}
            onKeyDown={(e) => {
              if (e.key === 'Enter' || e.key === ' ') startEditing()
            }}
            role="button"
            tabIndex={disabled ? -1 : 0}
            aria-label={`Edit ${descriptor.name} value`}
          >
            {value}
          </span>
        )}
        <button
          type="button"
          className="control-slider__reset"
          onClick={onReset}
          disabled={disabled}
          aria-label={`Reset ${descriptor.name}`}
        >
          Reset
        </button>
      </div>
      {error && (
        <p className="control-slider__error" role="alert">
          {error}
        </p>
      )}
    </div>
  )
}
