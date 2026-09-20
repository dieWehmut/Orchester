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
 * region without reaching into a class name. The shell owns the three columns
 * it lays out, so the rail, the transcript and the inspector each carry the
 * attribute the spec gives them in addition to the landmark they already are.
 */

const IMPLICIT_ROLES: Record<string, string> = {
  NAV: 'navigation',
  MAIN: 'main',
  ASIDE: 'complementary',
}

function roleOf(element: Element): string {
  return element.getAttribute('role') ?? IMPLICIT_ROLES[element.tagName] ?? element.tagName
}

function mountShell() {
  return mount(AppShell, {
    props: { sessionsTitle: 'Sessions', inspectorTitle: 'Inspector' },
    slots: {
      sessions: '<p>Sessions</p>',
      default: '<p>Transcript</p>',
      inspector: '<p>Inspector</p>',
    },
  })
}

describe('AppShell region contract', () => {
  it('names the rail, the transcript and the inspector with the spec attributes', () => {
    const wrapper = mountShell()

    // `get` already fails when the element is missing, so reaching it is the
    // assertion; the names are here so a rename shows up as a named failure.
    expect(wrapper.get('[data-rail]').attributes('data-rail-appearance')).toBeTruthy()
    expect(wrapper.get('[data-transcript]').isVisible()).toBe(true)
    expect(wrapper.get('[data-inspector]').attributes('data-inspector-tab')).toBe('context')
  })

  it('keeps the three regions the landmarks they already were', () => {
    const wrapper = mountShell()

    expect(roleOf(wrapper.get('[data-rail]').element)).toBe('navigation')
    expect(roleOf(wrapper.get('[data-transcript]').element)).toBe('main')
    expect(roleOf(wrapper.get('[data-inspector]').element)).toBe('complementary')
  })

  it('carries the rail appearance and the inspector state as attributes', () => {
    const wrapper = mount(AppShell, {
      props: {
        sessionsTitle: 'Sessions',
        inspectorTitle: 'Inspector',
        railAppearance: 'translucent',
        inspectorOpen: true,
        inspectorFullWidth: true,
        inspectorTab: 'changes',
      },
      slots: { default: '<p>Transcript</p>' },
    })

    expect(wrapper.get('[data-rail]').attributes('data-rail-appearance')).toBe('translucent')
    expect(wrapper.get('[data-inspector]').attributes('data-inspector-full-width')).toBe('true')
    expect(wrapper.get('[data-inspector]').attributes('data-inspector-tab')).toBe('changes')
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

  it('keeps the inspector out of the transcript region', () => {
    const wrapper = mountShell()

    const transcript = wrapper.get('[data-transcript]').element
    expect(transcript.contains(wrapper.get('[data-inspector]').element)).toBe(false)
    expect(transcript.contains(wrapper.get('[data-rail]').element)).toBe(false)
  })
})
