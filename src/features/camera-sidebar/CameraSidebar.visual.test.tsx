import { expect, test } from 'vitest'
import { page, renderVisual } from '../../test-utils/visual'
import type { CameraDevice } from '../../types/camera'
import { CameraEntry } from './CameraEntry'
import './CameraSidebar.css'
import './CameraEntry.css'

const devices: CameraDevice[] = [
  { id: 'cam-1', name: 'HD Webcam C920', devicePath: '/dev/video0', isConnected: true },
  { id: 'cam-2', name: 'USB Camera', devicePath: '/dev/video1', isConnected: true },
  { id: 'cam-3', name: 'FaceTime HD', devicePath: '/dev/video2', isConnected: true },
]

test('sidebar with camera entries matches baseline', async () => {
  await renderVisual(
    <nav className="camera-sidebar">
      <div role="listbox" className="camera-sidebar__list">
        {devices.map((device, i) => (
          <CameraEntry key={device.id} device={device} isSelected={i === 0} onSelect={() => {}} />
        ))}
      </div>
    </nav>,
  )

  const sidebar = page.elementLocator(document.querySelector('.camera-sidebar') as Element)
  await expect.element(sidebar).toMatchScreenshot()
})
