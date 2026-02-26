import { useEffect } from 'react'
import { CameraSidebar, listCameras, useCameraStore, useHotplug } from './features/camera-sidebar'
import './App.css'

function App() {
  const setCameras = useCameraStore((s) => s.setCameras)

  useEffect(() => {
    listCameras()
      .then(setCameras)
      .catch((err: unknown) => {
        console.error('Failed to list cameras:', err)
      })
  }, [setCameras])

  useHotplug()

  return (
    <div className="app-layout">
      <CameraSidebar />
      <main className="app-main">
        <h1>Webcam Settings</h1>
      </main>
    </div>
  )
}

export default App
