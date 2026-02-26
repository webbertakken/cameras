import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { EmptyState } from './EmptyState'

describe('EmptyState', () => {
  it('renders "No cameras found" message', () => {
    render(<EmptyState />)
    expect(screen.getByText('No cameras found')).toBeInTheDocument()
  })

  it('renders a camera-off icon', () => {
    render(<EmptyState />)
    expect(screen.getByTestId('camera-off-icon')).toBeInTheDocument()
  })

  it('is accessible with appropriate role and label', () => {
    render(<EmptyState />)
    const region = screen.getByRole('status')
    expect(region).toHaveAccessibleName('No cameras found')
  })
})
