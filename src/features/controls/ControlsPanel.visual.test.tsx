import { expect, test } from 'vitest'
import { page, renderVisual } from '../../test-utils/visual'
import type { ControlDescriptor } from '../../types/camera'
import { AccordionSection } from './AccordionSection'
import { ControlRenderer } from './ControlRenderer'
import './ControlsPanel.css'
import './AccordionSection.css'
import './ControlSlider.css'
import './ControlToggle.css'
import './ControlSelect.css'

const imageControls: ControlDescriptor[] = [
  {
    id: 'brightness',
    name: 'Brightness',
    controlType: 'slider',
    group: 'image',
    min: 0,
    max: 255,
    step: 1,
    default: 128,
    current: 128,
    flags: { supportsAuto: false, isAutoEnabled: false, isReadOnly: false },
    supported: true,
  },
  {
    id: 'contrast',
    name: 'Contrast',
    controlType: 'slider',
    group: 'image',
    min: 0,
    max: 255,
    step: 1,
    default: 128,
    current: 100,
    flags: { supportsAuto: false, isAutoEnabled: false, isReadOnly: false },
    supported: true,
  },
]

const exposureControls: ControlDescriptor[] = [
  {
    id: 'auto-exposure',
    name: 'Auto exposure',
    controlType: 'toggle',
    group: 'exposure',
    min: 0,
    max: 1,
    step: 1,
    default: 1,
    current: 1,
    flags: { supportsAuto: false, isAutoEnabled: false, isReadOnly: false },
    supported: true,
  },
  {
    id: 'white-balance',
    name: 'White balance',
    controlType: 'select',
    group: 'exposure',
    min: 0,
    max: 4,
    step: 1,
    default: 0,
    current: 2,
    flags: { supportsAuto: true, isAutoEnabled: false, isReadOnly: false },
    supported: true,
  },
]

test('controls panel with accordion sections matches baseline', async () => {
  await renderVisual(
    <section aria-label="Camera controls" role="region" className="controls-panel">
      <AccordionSection label="Image" sectionId="image" defaultExpanded>
        {imageControls.map((desc) => (
          <ControlRenderer
            key={desc.id}
            descriptor={desc}
            value={desc.current}
            cameraName="Test Camera"
            onChange={() => {}}
            onReset={() => {}}
          />
        ))}
      </AccordionSection>
      <AccordionSection label="Exposure & white balance" sectionId="exposure">
        {exposureControls.map((desc) => (
          <ControlRenderer
            key={desc.id}
            descriptor={desc}
            value={desc.current}
            cameraName="Test Camera"
            onChange={() => {}}
            onReset={() => {}}
          />
        ))}
      </AccordionSection>
    </section>,
  )

  const panel = page.elementLocator(document.querySelector('.controls-panel') as Element)
  await expect.element(panel).toMatchScreenshot()
})
