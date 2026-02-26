import { type ReactNode, useId, useState } from 'react'
import './AccordionSection.css'

interface AccordionSectionProps {
  label: string
  sectionId?: string
  defaultExpanded?: boolean
  children: ReactNode
}

export function AccordionSection({
  label,
  sectionId,
  defaultExpanded = false,
  children,
}: AccordionSectionProps) {
  const [expanded, setExpanded] = useState(defaultExpanded)
  const generatedId = useId()
  const baseId = sectionId ?? generatedId
  const buttonId = `accordion-btn-${baseId}`
  const regionId = `accordion-region-${baseId}`

  return (
    <div className="accordion-section">
      <h3 className="accordion-section__header">
        <button
          type="button"
          id={buttonId}
          className="accordion-section__trigger"
          aria-expanded={expanded}
          aria-controls={regionId}
          onClick={() => setExpanded((prev) => !prev)}
        >
          <span className="accordion-section__label">{label}</span>
          <span
            className={`accordion-section__icon${expanded ? ' accordion-section__icon--expanded' : ''}`}
            aria-hidden="true"
          >
            &#9654;
          </span>
        </button>
      </h3>
      <div
        id={regionId}
        role="region"
        aria-labelledby={buttonId}
        className={`accordion-section__content${expanded ? '' : ' accordion-section__content--collapsed'}`}
        hidden={!expanded}
      >
        {children}
      </div>
    </div>
  )
}
