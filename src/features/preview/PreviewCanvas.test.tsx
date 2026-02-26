import { describe, expect, it } from 'vitest'
import { render, screen } from '@testing-library/react'
import { PreviewCanvas } from './PreviewCanvas.tsx'

describe('PreviewCanvas', () => {
  it('renders a container with role="img" when no frame', () => {
    render(<PreviewCanvas frameSrc={null} />)
    expect(screen.getByRole('img')).toBeInTheDocument()
  })

  it('shows "No preview" placeholder when no frame data', () => {
    render(<PreviewCanvas frameSrc={null} />)
    expect(screen.getByText('No preview')).toBeInTheDocument()
  })

  it('displays an image when given base64 JPEG data', () => {
    const src = 'data:image/jpeg;base64,/9j/4AAQ'
    render(<PreviewCanvas frameSrc={src} />)
    const img = screen.getByRole('img') as HTMLImageElement
    expect(img.tagName).toBe('IMG')
    expect(img.src).toBe(src)
  })

  it('has correct alt text for accessibility', () => {
    const src = 'data:image/jpeg;base64,/9j/4AAQ'
    render(<PreviewCanvas frameSrc={src} />)
    expect(screen.getByAltText('Camera preview')).toBeInTheDocument()
  })

  it('renders image with preview-canvas__image class', () => {
    const src = 'data:image/jpeg;base64,/9j/4AAQ'
    render(<PreviewCanvas frameSrc={src} />)
    const img = screen.getByAltText('Camera preview')
    expect(img.className).toContain('preview-canvas__image')
  })
})
