import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const css = readFileSync(resolve(__dirname, 'tokens.css'), 'utf-8')

describe('design tokens', () => {
  it('defaults to dark theme on :root to prevent flash', () => {
    expect(css).toContain(':root')
    expect(css).toMatch(/:root\s*\{[\s\S]*?--colour-bg:\s*#1c1c1e/)
    expect(css).toMatch(/:root\s*\{[\s\S]*?--colour-text-primary:\s*#f5f5f7/)
  })

  it('defines light theme tokens under [data-theme="light"]', () => {
    expect(css).toContain("[data-theme='light']")
    expect(css).toContain('--colour-bg: #ffffff')
    expect(css).toContain('--colour-text-primary: #1d1d1f')
    expect(css).toContain('--colour-accent: #0071e3')
  })

  it('defines dark theme tokens under [data-theme="dark"]', () => {
    expect(css).toContain("[data-theme='dark']")
    expect(css).toContain('--colour-bg: #1c1c1e')
    expect(css).toContain('--colour-text-primary: #f5f5f7')
    expect(css).toContain('--colour-accent: #0a84ff')
  })

  it('has different bg values for light and dark themes', () => {
    const lightMatch = css.match(/\[data-theme='light'\][\s\S]*?--colour-bg:\s*([^;]+);/)
    const darkMatch = css.match(/\[data-theme='dark'\][\s\S]*?--colour-bg:\s*([^;]+);/)

    expect(lightMatch).toBeTruthy()
    expect(darkMatch).toBeTruthy()

    const lightBg = (lightMatch as RegExpMatchArray)[1].trim()
    const darkBg = (darkMatch as RegExpMatchArray)[1].trim()
    expect(lightBg).not.toBe(darkBg)
  })

  it('defines spacing scale tokens', () => {
    expect(css).toContain('--space-1: 4px')
    expect(css).toContain('--space-2: 8px')
    expect(css).toContain('--space-4: 16px')
  })

  it('defines typography tokens', () => {
    expect(css).toContain('--font-size-md: 15px')
    expect(css).toContain('--radius-md: 8px')
    expect(css).toContain('--font-family:')
  })

  it('defines border and status tokens', () => {
    expect(css).toContain('--colour-border:')
    expect(css).toContain('--colour-border-focus:')
    expect(css).toContain('--colour-success:')
    expect(css).toContain('--colour-warning:')
    expect(css).toContain('--colour-error:')
  })

  it('defines shadow tokens', () => {
    expect(css).toContain('--shadow-sm:')
    expect(css).toContain('--shadow-md:')
  })
})
