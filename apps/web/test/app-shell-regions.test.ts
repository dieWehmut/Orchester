import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import AppShell from '../src/components/layout/AppShell.vue'

/**
 * The shell's own region contract, section 2 of the design spec.
 *
 * The spec letters the shell's regions and gives each one the attribute that
 * names it, because those attributes are how the styles and the tests reach a
 * region without reaching into a class name. The shell lays out **two** columns
 * - the rail and the transcript - because the reference it is built against has
 * no right sidebar: the run's own surfaces are drawn in the bottom panel, which
 * is a region of the transcript column rather than a column of its own.
 */

const IMPLICIT_ROLES: Record<string, string> = {
  NAV: 'navigation',
  MAIN: 'main',
}

function roleOf(element: Element): string {
  return element.getAttribute('role') ?? IMPLICIT_ROLES[element.tagName] ?? element.tagName
}

function mountShell() {
  return mount(AppShell, {
    props: { sessionsTitle: 'Sessions' },
    slots: {
      sessions: '<p>Sessions</p>',
      default: '<p>Transcript</p>',
    },
  })
}

describe('AppShell region contract', () => {
  it('names the rail and the transcript with the spec attributes', () => {
    const wrapper = mountShell()

    // `get` already fails when the element is missing, so reaching it is the
    // assertion; the names are here so a rename shows up as a named failure.
    expect(wrapper.get('[data-rail]').attributes('data-rail-appearance')).toBeTruthy()
    expect(wrapper.get('[data-transcript]').isVisible()).toBe(true)
  })

  it('draws no right column: the transcript is the last region', () => {
    const wrapper = mountShell()

    // The reference's conversation runs to the window's edge. A third column
    // coming back would be a regression against what this shell was changed to
    // match, so its absence is the assertion.
    expect(wrapper.find('[data-inspector]').exists()).toBe(false)
    expect(wrapper.find('[data-pane="inspector"]').exists()).toBe(false)
    expect(wrapper.findAll('[data-pane]').map((node) => node.attributes('data-pane'))).toEqual([
      'sessions',
      'transcript',
    ])
  })

  it('keeps the two regions the landmarks they already were', () => {
    const wrapper = mountShell()

    expect(roleOf(wrapper.get('[data-rail]').element)).toBe('navigation')
    expect(roleOf(wrapper.get('[data-transcript]').element)).toBe('main')
  })

  it('carries the rail appearance as an attribute', () => {
    const wrapper = mount(AppShell, {
      props: { sessionsTitle: 'Sessions', railAppearance: 'translucent' },
      slots: { default: '<p>Transcript</p>' },
    })

    expect(wrapper.get('[data-rail]').attributes('data-rail-appearance')).toBe('translucent')
  })

  it('is the shell the workspace view mounts, not a second copy of it', () => {
    // The regions only exist for the product if the product is drawn inside
    // them, so the view that owns the workspace has to mount this shell.
    const view = readFileSync(
      resolve(process.cwd(), 'src', 'views', 'WorkspaceView.vue'),
      'utf8',
    )

    expect(view).toContain("from '../components/layout/AppShell.vue'")
    expect(view).toContain('<AppShell')
    expect(view).not.toContain('WorkspaceResponsive')
  })

  it('keeps the rail out of the transcript region', () => {
    const wrapper = mountShell()

    const transcript = wrapper.get('[data-transcript]').element
    expect(transcript.contains(wrapper.get('[data-rail]').element)).toBe(false)
  })
})