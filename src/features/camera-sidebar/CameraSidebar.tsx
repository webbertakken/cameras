import { CameraEntry } from './CameraEntry'
import './CameraSidebar.css'
import { EmptyState } from './EmptyState'
import { useCameraStore } from './store'

export function CameraSidebar() {
  const cameras = useCameraStore((s) => s.cameras)
  const selectedId = useCameraStore((s) => s.selectedId)
  const selectCamera = useCameraStore((s) => s.selectCamera)

  if (cameras.length === 0) {
    return (
      <nav aria-label="Camera list" className="camera-sidebar">
        <EmptyState />
      </nav>
    )
  }

  return (
    <nav aria-label="Camera list" className="camera-sidebar">
      <div role="listbox" aria-label="Cameras" className="camera-sidebar__list">
        {cameras.map((device) => (
          <CameraEntry
            key={device.id}
            device={device}
            isSelected={selectedId === device.id}
            onSelect={selectCamera}
          />
        ))}
      </div>
    </nav>
  )
}
