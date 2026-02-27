import { describe, expect, it } from 'vitest'
import { render, screen } from '@testing-library/react'
import { PreviewCanvas } from './PreviewCanvas.tsx'

describe('PreviewCanvas', () => {
  it('renders a container with role="img" when no frame and not loading', () => {
    render(<PreviewCanvas frameSrc={null} isLoading={false} />)
    expect(screen.getByRole('img')).toBeInTheDocument()
  })

  it('shows "No preview" placeholder when no frame and not loading', () => {
    render(<PreviewCanvas frameSrc={null} isLoading={false} />)
    expect(screen.getByText('No preview')).toBeInTheDocument()
  })

  it('shows loading state when frameSrc is null and isLoading is true', () => {
    render(<PreviewCanvas frameSrc={null} isLoading={true} />)
    expect(screen.getByText('Starting preview...')).toBeInTheDocument()
  })

  it('displays an image when given a frame src', () => {
    const src = 'blob:http://localhost/fake-blob'
    render(<PreviewCanvas frameSrc={src} isLoading={false} />)
    const img = screen.getByRole('img') as HTMLImageElement
    expect(img.tagName).toBe('IMG')
    expect(img.src).toBe(src)
  })

  it('has correct alt text for accessibility', () => {
    const src = 'blob:http://localhost/fake-blob'
    render(<PreviewCanvas frameSrc={src} isLoading={false} />)
    expect(screen.getByAltText('Camera preview')).toBeInTheDocument()
  })

  it('renders image with preview-canvas__image class', () => {
    const src = 'blob:http://localhost/fake-blob'
    render(<PreviewCanvas frameSrc={src} isLoading={false} />)
    const img = screen.getByAltText('Camera preview')
    expect(img.className).toContain('preview-canvas__image')
  })

  it('displays error message when error is provided', () => {
    render(<PreviewCanvas frameSrc={null} isLoading={false} error="Camera is in use" />)
    expect(screen.getByText('Preview unavailable')).toBeInTheDocument()
    expect(screen.getByText('Camera is in use')).toBeInTheDocument()
  })

  it('shows error state over loading state when both present', () => {
    render(<PreviewCanvas frameSrc={null} isLoading={true} error="Device busy" />)
    expect(screen.getByText('Preview unavailable')).toBeInTheDocument()
    expect(screen.queryByText('Starting preview...')).not.toBeInTheDocument()
  })

  it('applies loading spinner class', () => {
    const { container } = render(<PreviewCanvas frameSrc={null} isLoading={true} />)
    expect(container.querySelector('.preview-canvas__spinner')).toBeInTheDocument()
  })
})
