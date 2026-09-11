import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

import { describe, expect, it } from 'vitest'

const root = resolve(process.cwd())
const viteConfig = readFileSync(resolve(root, 'vite.config.ts'), 'utf8')
const appCss = readFileSync(resolve(root, 'src/styles/app.css'), 'utf8')

describe('WebUI Tailwind foundation', () => {
  it('registers the Tailwind Vite plugin without changing the local port', () => {
    expect(viteConfig).toContain("from '@tailwindcss/vite'")
    expect(viteConfig).toContain('tailwindcss()')
    expect(viteConfig).toContain("host: '127.0.0.1'")
    expect(viteConfig).toContain('port: 4173')
  })

  it('loads a dedicated Tailwind entry through the app stylesheet', () => {
    expect(appCss).toContain("@import './tailwind.css';")
    const tailwindCss = readFileSync(resolve(root, 'src/styles/tailwind.css'), 'utf8')
    expect(tailwindCss).toContain('@import "tailwindcss";')
    expect(tailwindCss).toContain('@theme inline')
    expect(tailwindCss).toContain('--color-orchester-bg-base:')
    expect(tailwindCss).toContain('--font-orchester-mono: var(--font-mono)')
    expect(tailwindCss).not.toContain('--font-mono: var(--font-mono)')
  })
})
