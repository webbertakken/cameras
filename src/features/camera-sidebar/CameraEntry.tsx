import type { CameraDevice } from '../../types/camera'
import './CameraEntry.css'

interface CameraEntryProps {
  device: CameraDevice
  isSelected: boolean
  onSelect: (id: string) => void
  thumbnailSrc?: string | null
}

export function CameraEntry({ device, isSelected, onSelect, thumbnailSrc }: CameraEntryProps) {
  return (
    <div
      role="option"
      aria-label={device.name}
      aria-selected={isSelected}
      className={`camera-entry${isSelected ? ' camera-entry--selected' : ''}`}
      tabIndex={0}
      onClick={() => onSelect(device.id)}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault()
          onSelect(device.id)
        }
      }}
    >
      <div className="camera-entry__thumbnail" data-testid="camera-thumbnail">
        {thumbnailSrc ? (
          <img
            className="camera-entry__thumbnail-img"
            src={thumbnailSrc}
            alt={`${device.name} preview`}
          />
        ) : (
          <svg
            className="camera-entry__icon"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.5"
            aria-hidden="true"
          >
            <path d="M15.75 10.5l4.72-4.72a.75.75 0 011.28.53v11.38a.75.75 0 01-1.28.53l-4.72-4.72M4.5 18.75h9a2.25 2.25 0 002.25-2.25v-9a2.25 2.25 0 00-2.25-2.25h-9A2.25 2.25 0 002.25 7.5v9a2.25 2.25 0 002.25 2.25z" />
          </svg>
        )}
      </div>
      <span className="camera-entry__name">{device.name}</span>
    </div>
  )
}
