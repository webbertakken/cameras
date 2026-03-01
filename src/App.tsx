import { useEffect } from 'react'
import { CameraSidebar, listCameras, useCameraStore, useHotplug } from './features/camera-sidebar'
import { ControlsPanel } from './features/controls/ControlsPanel'
import { ToastContainer } from './features/notifications'
import { PreviewCanvas } from './features/preview/PreviewCanvas'
import { usePreview } from './features/preview/usePreview'
import './App.css'

function App() {
  const setCameras = useCameraStore((s) => s.setCameras)
  const selectedCamera = useCameraStore((s) => s.selectedCamera())

  useEffect(() => {
    listCameras()
      .then(setCameras)
      .catch((err: unknown) => {
        console.error('Failed to list cameras:', err)
      })
  }, [setCameras])

  useHotplug()

  const preview = usePreview(selectedCamera?.id ?? null)

  // Start preview when a camera is selected, stop when deselected or changed.
  // No ref guard — the backend handles deduplication, and this must survive
  // React 18 strict mode's mount→cleanup→mount cycle.
  useEffect(() => {
    const cameraId = selectedCamera?.id ?? null

    if (cameraId) {
      preview.start(640, 480, 30).catch((err: unknown) => {
        console.error('Failed to start preview:', err)
      })
    }

    return () => {
      preview.stop().catch((err: unknown) => {
        console.error('Failed to stop preview:', err)
      })
    }
  }, [selectedCamera?.id]) // eslint-disable-line react-hooks/exhaustive-deps

  return (
    <div className="app-layout">
      <CameraSidebar />
      <main className="app-main">
        {selectedCamera ? (
          <>
            <PreviewCanvas
              frameSrc={preview.frameSrc}
              isLoading={preview.isActive && !preview.frameSrc}
              error={preview.error}
            />
            <ControlsPanel cameraId={selectedCamera.id} cameraName={selectedCamera.name} />
          </>
        ) : (
          <div className="app-main__placeholder">
            <p>Select a camera to get started</p>
          </div>
        )}
      </main>
      <ToastContainer />
    </div>
  )
}

export default App
