import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import { AccordionSection } from './AccordionSection'

describe('AccordionSection', () => {
  // --- Rendering ---

  it('renders section header with group label', () => {
    render(
      <AccordionSection label="Image" defaultExpanded>
        <p>Controls here</p>
      </AccordionSection>,
    )
    expect(screen.getByRole('button', { name: 'Image' })).toBeInTheDocument()
  })

  it('renders children when expanded', () => {
    render(
      <AccordionSection label="Image" defaultExpanded>
        <p>Controls here</p>
      </AccordionSection>,
    )
    expect(screen.getByText('Controls here')).toBeVisible()
  })

  it('hides children when collapsed', () => {
    render(
      <AccordionSection label="Image" defaultExpanded={false}>
        <p>Controls here</p>
      </AccordionSection>,
    )
    expect(screen.queryByText('Controls here')).not.toBeVisible()
  })

  // --- Interaction ---

  it('toggles expanded state on header click', async () => {
    const user = userEvent.setup()
    render(
      <AccordionSection label="Image" defaultExpanded>
        <p>Controls here</p>
      </AccordionSection>,
    )
    const button = screen.getByRole('button', { name: 'Image' })
    await user.click(button)
    expect(screen.queryByText('Controls here')).not.toBeVisible()
    await user.click(button)
    expect(screen.getByText('Controls here')).toBeVisible()
  })

  it('supports multiple sections open simultaneously', () => {
    render(
      <>
        <AccordionSection label="Image" defaultExpanded>
          <p>Image controls</p>
        </AccordionSection>
        <AccordionSection label="Exposure" defaultExpanded>
          <p>Exposure controls</p>
        </AccordionSection>
      </>,
    )
    expect(screen.getByText('Image controls')).toBeVisible()
    expect(screen.getByText('Exposure controls')).toBeVisible()
  })

  // --- Accessibility ---

  it('uses aria-expanded on the header button', () => {
    render(
      <AccordionSection label="Image" defaultExpanded>
        <p>Content</p>
      </AccordionSection>,
    )
    expect(screen.getByRole('button', { name: 'Image' })).toHaveAttribute('aria-expanded', 'true')
  })

  it('sets aria-expanded to false when collapsed', () => {
    render(
      <AccordionSection label="Image" defaultExpanded={false}>
        <p>Content</p>
      </AccordionSection>,
    )
    expect(screen.getByRole('button', { name: 'Image' })).toHaveAttribute('aria-expanded', 'false')
  })

  it('has aria-controls pointing to the content region', () => {
    render(
      <AccordionSection label="Image" sectionId="image" defaultExpanded>
        <p>Content</p>
      </AccordionSection>,
    )
    const button = screen.getByRole('button', { name: 'Image' })
    const region = screen.getByRole('region')
    expect(button).toHaveAttribute('aria-controls', region.id)
  })

  it('content region has role="region" with aria-labelledby', () => {
    render(
      <AccordionSection label="Image" sectionId="image" defaultExpanded>
        <p>Content</p>
      </AccordionSection>,
    )
    const region = screen.getByRole('region')
    const button = screen.getByRole('button', { name: 'Image' })
    expect(region).toHaveAttribute('aria-labelledby', button.id)
  })

  it('header is toggleable via Enter', async () => {
    const user = userEvent.setup()
    render(
      <AccordionSection label="Image" defaultExpanded>
        <p>Content</p>
      </AccordionSection>,
    )
    const button = screen.getByRole('button', { name: 'Image' })
    button.focus()
    await user.keyboard('{Enter}')
    expect(button).toHaveAttribute('aria-expanded', 'false')
  })

  it('header is toggleable via Space', async () => {
    const user = userEvent.setup()
    render(
      <AccordionSection label="Image" defaultExpanded>
        <p>Content</p>
      </AccordionSection>,
    )
    const button = screen.getByRole('button', { name: 'Image' })
    button.focus()
    await user.keyboard(' ')
    expect(button).toHaveAttribute('aria-expanded', 'false')
  })
})
