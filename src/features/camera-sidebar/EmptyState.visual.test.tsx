import { expect, test } from 'vitest'
import { page, renderVisual } from '../../test-utils/visual'
import { EmptyState } from './EmptyState'
import './CameraSidebar.css'

test('empty state matches baseline', async () => {
  await renderVisual(
    <nav className="camera-sidebar">
      <EmptyState />
    </nav>,
  )

  const sidebar = page.elementLocator(document.querySelector('.camera-sidebar') as Element)
  await expect.element(sidebar).toMatchScreenshot()
})
