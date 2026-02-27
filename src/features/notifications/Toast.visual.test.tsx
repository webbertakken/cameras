import { expect, test } from 'vitest'
import { page, renderVisual } from '../../test-utils/visual'
import { Toast } from './Toast'
import './Toast.css'

test('toast notifications match baseline', async () => {
  await renderVisual(
    <div
      className="visual-test-wrapper"
      style={{ display: 'flex', flexDirection: 'column', gap: '8px', width: '360px' }}
    >
      <Toast id="1" message="Camera connected successfully" type="success" onDismiss={() => {}} />
      <Toast id="2" message="Firmware update available" type="info" onDismiss={() => {}} />
    </div>,
  )

  const wrapper = page.elementLocator(document.querySelector('.visual-test-wrapper') as Element)
  await expect.element(wrapper).toMatchScreenshot()
})
