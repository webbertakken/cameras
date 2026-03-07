import type { CameraDevice } from '../../types/camera'
import './CameraEntry.css'

interface CameraEntryProps {
  device: CameraDevice
  isSelected: boolean
  onSelect: (id: string) => void
  thumbnailSrc?: string | null
  /** Whether this device has an active preview session. */
  hasPreview?: boolean
  /** Whether the virtual camera output is active. */
  isVirtualCameraActive?: boolean
  /** Called when the virtual camera toggle is clicked. */
  onToggleVirtualCamera?: (id: string) => void
}

export function CameraEntry({
  device,
  isSelected,
  onSelect,
  thumbnailSrc,
  hasPreview = false,
  isVirtualCameraActive = false,
  onToggleVirtualCamera,
}: CameraEntryProps) {
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
      {onToggleVirtualCamera && (
        <button
          type="button"
          className={`camera-entry__vcam-toggle${isVirtualCameraActive ? ' camera-entry__vcam-toggle--active' : ''}`}
          disabled={!hasPreview}
          title={isVirtualCameraActive ? 'Stop virtual camera' : 'Expose as virtual camera'}
          aria-label={isVirtualCameraActive ? 'Stop virtual camera' : 'Expose as virtual camera'}
          onClick={(e) => {
            e.stopPropagation()
            onToggleVirtualCamera(device.id)
          }}
          onKeyDown={(e) => e.stopPropagation()}
        >
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.5"
            width="16"
            height="16"
            aria-hidden="true"
          >
            <path d="M6 20.25h12m-7.5-3v3m3-3v3m-10.125-3h17.25c.621 0 1.125-.504 1.125-1.125V4.875c0-.621-.504-1.125-1.125-1.125H3.375c-.621 0-1.125.504-1.125 1.125v11.25c0 .621.504 1.125 1.125 1.125z" />
          </svg>
        </button>
      )}
    </div>
  )
}
