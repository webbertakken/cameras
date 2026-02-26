import { expect, test } from 'vitest'
import { renderVisual } from '../../test-utils/visual'
import type { ControlDescriptor } from '../../types/camera'
import { ControlSlider } from './ControlSlider'
import './ControlSlider.css'

const unsupportedControl: ControlDescriptor = {
  id: 'gain',
  name: 'Gain',
  controlType: 'slider',
  group: 'image',
  min: 0,
  max: 255,
  step: 1,
  default: 128,
  current: 64,
  flags: { supportsAuto: false, isAutoEnabled: false, isReadOnly: false },
  supported: false,
}

test('disabled slider matches baseline', async () => {
  const { container } = await renderVisual(
    <div style={{ width: '400px' }}>
      <ControlSlider
        descriptor={unsupportedControl}
        value={64}
        onChange={() => {}}
        onReset={() => {}}
        disabled
        cameraName="Test Camera"
      />
    </div>,
  )

  await expect.element(container).toMatchScreenshot()
})
