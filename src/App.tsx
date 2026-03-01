import { useEffect, useRef } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { CameraSidebar, listCameras, useCameraStore, useHotplug } from './features/camera-sidebar'
import { ControlsPanel } from './features/controls/ControlsPanel'
import { ToastContainer } from './features/notifications'
import { PreviewCanvas } from './features/preview/PreviewCanvas'
import { usePreview } from './features/preview/usePreview'
import './App.css'

function App() {
  const setCameras = useCameraStore((s) => s.setCameras)
  const selectedCamera = useCameraStore(
    useShallow((s) => s.cameras.find((c) => c.id === s.selectedId)),
  )

  useEffect(() => {
    listCameras()
      .then(setCameras)
      .catch((err: unknown) => {
        console.error('Failed to list cameras:', err)
      })
  }, [setCameras])

  useHotplug()

  const preview = usePreview(selectedCamera?.id ?? null)

  // Keep refs to the latest start/stop so the effect only re-fires on camera
  // ID changes, not when the callback references are recreated on re-render.
  const startRef = useRef(preview.start)
  const stopRef = useRef(preview.stop)
  useEffect(() => {
    startRef.current = preview.start
    stopRef.current = preview.stop
  })

  // Start preview when a camera is selected, stop when deselected or changed
  useEffect(() => {
    const cameraId = selectedCamera?.id ?? null

    if (cameraId) {
      startRef.current(640, 480, 30).catch((err: unknown) => {
        console.error('Failed to start preview:', err)
      })
    }

    return () => {
      stopRef.current().catch((err: unknown) => {
        console.error('Failed to stop preview:', err)
      })
    }
  }, [selectedCamera?.id])

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
