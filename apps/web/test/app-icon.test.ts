import { existsSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import OrchesterMark from '../src/components/run/OrchesterMark.vue'
import AppRail from '../src/components/layout/AppRail.vue'

function fromPackageRoot(relativePath: string): string {
  return resolve(process.cwd(), relativePath)
}

describe('Orchester app icon', () => {
  it('links the icon assets from the document head', () => {
    const html = readFileSync(fromPackageRoot('index.html'), 'utf8')

    expect(html).toMatch(/<link[^>]+rel="icon"[^>]+href="\/favicon\.png"/)
    expect(html).toMatch(/<link[^>]+rel="apple-touch-icon"[^>]+href="\/apple-touch-icon\.png"/)
  })

  it('ships every icon asset the app points at', () => {
    for (const asset of [
      'public/favicon.png',
      'public/apple-touch-icon.png',
      'public/icon-512.png',
      'src/assets/orchester-mark.png',
    ]) {
      expect(existsSync(fromPackageRoot(asset)), asset).toBe(true)
    }
  })

  it('renders the brand mark as the shipped artwork', () => {
    const wrapper = mount(OrchesterMark, { props: { size: 96 } })

    const mark = wrapper.get('[data-orchester-mark]')
    const image = mark.get('img')
    expect(mark.attributes('aria-hidden')).toBe('true')
    expect(image.attributes('src')).toContain('orchester-mark')
    expect(image.attributes('width')).toBe('96')
    expect(image.attributes('height')).toBe('96')
    expect(image.attributes('alt')).toBe('')
  })

  it('uses the same artwork for the rail badge instead of a letter tile', () => {
    const wrapper = mount(AppRail, {
      props: {
        productName: 'Orchester',
        newSessionLabel: 'New session',
        projectsLabel: 'Projects',
        sessionsLabel: 'Sessions',
        fleetLabel: 'Agents',
      },
    })

    const badge = wrapper.get('[data-rail-mark]')
    expect(badge.element.tagName).toBe('IMG')
    expect(badge.attributes('src')).toContain('orchester-mark')
    expect(badge.attributes('alt')).toBe('')
  })
})