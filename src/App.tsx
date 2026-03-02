import { useEffect, useRef } from 'react'
import { invoke } from '@tauri-apps/api/core'
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

  // List cameras and start all backend capture sessions on mount
  useEffect(() => {
    listCameras()
      .then(setCameras)
      .catch((err: unknown) => {
        console.error('Failed to list cameras:', err)
      })

    invoke('start_all_previews').catch((err: unknown) => {
      console.error('Failed to start all previews:', err)
    })
  }, [setCameras])

  useHotplug()

  const preview = usePreview(selectedCamera?.id ?? null)

  // Keep a ref to the latest start/stop so the effect only re-fires on
  // camera ID changes, not when the callback references are recreated.
  const startRef = useRef(preview.start)
  const stopRef = useRef(preview.stop)
  useEffect(() => {
    startRef.current = preview.start
    stopRef.current = preview.stop
  })

  // Start the display loop when a camera is selected, stop when changed
  useEffect(() => {
    const cameraId = selectedCamera?.id ?? null

    if (cameraId) {
      startRef.current()
    }

    return () => {
      stopRef.current()
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
