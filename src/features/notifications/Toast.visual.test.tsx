import { expect, test } from 'vitest'
import { renderVisual } from '../../test-utils/visual'
import { Toast } from './Toast'
import './Toast.css'

test('toast notifications match baseline', async () => {
  const { container } = await renderVisual(
    <div style={{ display: 'flex', flexDirection: 'column', gap: '8px', width: '360px' }}>
      <Toast id="1" message="Camera connected successfully" type="success" onDismiss={() => {}} />
      <Toast id="2" message="Firmware update available" type="info" onDismiss={() => {}} />
    </div>,
  )

  await expect.element(container).toMatchScreenshot()
})
