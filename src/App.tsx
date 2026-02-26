import { useEffect } from 'react'
import { CameraSidebar, listCameras, useCameraStore, useHotplug } from './features/camera-sidebar'
import { ControlsPanel } from './features/controls/ControlsPanel'
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

  const { frameSrc } = usePreview(selectedCamera?.id ?? null)

  return (
    <div className="app-layout">
      <CameraSidebar />
      <main className="app-main">
        {selectedCamera ? (
          <>
            <PreviewCanvas frameSrc={frameSrc} />
            <ControlsPanel cameraId={selectedCamera.id} cameraName={selectedCamera.name} />
          </>
        ) : (
          <div className="app-main__placeholder">
            <p>Select a camera to get started</p>
          </div>
        )}
      </main>
    </div>
  )
}

export default App
