import { useCallback, useEffect } from 'react'
import type { CameraDevice } from '../../types/camera'
import { useToastStore } from '../notifications/useToast'
import { useThumbnail } from '../preview/useThumbnail'
import { CameraEntry } from './CameraEntry'
import './CameraSidebar.css'
import { EmptyState } from './EmptyState'
import { usePreviewStateStore } from './preview-state-store'
import { useCameraStore } from './store'
import { useVirtualCameraStore } from './virtual-camera-store'

function CameraEntryWithThumbnail({
  device,
  isSelected,
  onSelect,
  hasPreview,
  isVirtualCameraActive,
  onToggleVirtualCamera,
}: {
  device: CameraDevice
  isSelected: boolean
  onSelect: (id: string) => void
  hasPreview: boolean
  isVirtualCameraActive: boolean
  onToggleVirtualCamera: (id: string) => void
}) {
  const thumbnailSrc = useThumbnail(device.id)
  return (
    <CameraEntry
      device={device}
      isSelected={isSelected}
      onSelect={onSelect}
      thumbnailSrc={thumbnailSrc}
      hasPreview={hasPreview}
      isVirtualCameraActive={isVirtualCameraActive}
      onToggleVirtualCamera={onToggleVirtualCamera}
    />
  )
}

export function CameraSidebar() {
  const cameras = useCameraStore((s) => s.cameras)
  const selectedId = useCameraStore((s) => s.selectedId)
  const selectCamera = useCameraStore((s) => s.selectCamera)
  const previewDeviceIds = usePreviewStateStore((s) => s.activeDeviceIds)
  const refreshPreviews = usePreviewStateStore((s) => s.refresh)
  const activeDevices = useVirtualCameraStore((s) => s.activeDevices)
  const toggleVcam = useVirtualCameraStore((s) => s.toggle)

  // Fetch active preview state on mount and when cameras change
  useEffect(() => {
    refreshPreviews().catch((err: unknown) => {
      console.error('Failed to refresh preview state:', err)
    })
  }, [cameras, refreshPreviews])

  const addToast = useToastStore((s) => s.addToast)

  const handleToggleVcam = useCallback(
    (deviceId: string) => {
      const isActive = activeDevices.has(deviceId)
      const action = isActive ? 'stop' : 'start'
      console.info(`[vcam] ${action} virtual camera for device '${deviceId}'`)
      toggleVcam(deviceId).catch((err: unknown) => {
        const detail = err instanceof Error ? err.message : String(err)
        console.error(`[vcam] Failed to ${action} virtual camera for device '${deviceId}':`, err)
        addToast(`Failed to ${action} virtual camera: ${detail}`, 'error')
      })
    },
    [toggleVcam, activeDevices, addToast],
  )

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
          <CameraEntryWithThumbnail
            key={device.id}
            device={device}
            isSelected={selectedId === device.id}
            onSelect={selectCamera}
            hasPreview={previewDeviceIds.has(device.id)}
            isVirtualCameraActive={activeDevices.has(device.id)}
            onToggleVirtualCamera={handleToggleVcam}
          />
        ))}
      </div>
    </nav>
  )
}
