import type { ControlDescriptor } from '../../types/camera'
import { ControlSelect } from './ControlSelect'
import { ControlSlider } from './ControlSlider'
import { ControlToggle } from './ControlToggle'

interface ControlRendererProps {
  descriptor: ControlDescriptor
  value: number
  cameraName: string
  onChange: (value: number) => void
  onReset: () => void
  error?: string
}

export function ControlRenderer({
  descriptor,
  value,
  cameraName,
  onChange,
  onReset,
  error,
}: ControlRendererProps) {
  const disabled = !descriptor.supported

  switch (descriptor.controlType) {
    case 'slider':
      return (
        <ControlSlider
          descriptor={descriptor}
          value={value}
          onChange={onChange}
          onReset={onReset}
          disabled={disabled}
          cameraName={cameraName}
          error={error}
        />
      )
    case 'toggle':
      return (
        <ControlToggle
          descriptor={descriptor}
          value={value}
          onChange={onChange}
          disabled={disabled}
          cameraName={cameraName}
        />
      )
    case 'select':
      return (
        <ControlSelect
          descriptor={descriptor}
          value={value}
          onChange={onChange}
          disabled={disabled}
          cameraName={cameraName}
        />
      )
  }
}
