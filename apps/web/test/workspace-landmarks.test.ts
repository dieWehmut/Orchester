import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import NotFoundView from '../src/views/NotFoundView.vue'
import WorkspaceHeader from '../src/components/layout/WorkspaceHeader.vue'
import AppShell from '../src/components/layout/AppShell.vue'

/**
 * The landmark half of the accessibility contract.
 *
 * Section 7 names one landmark per shell region: `nav` for the rail, `main`
 * for the transcript column and `complementary` for the inspector. A screen
 * reader user navigates by those regions, so a pane that is only a `<div>` is
 * a pane they cannot jump to, an `aside` where the contract says `nav` puts the
 * rail in the wrong list, and a route with no `main` at all is a page whose
 * content the reader cannot reach from the landmark menu.
 *
 * The roles are asserted from the element the region actually ships rather than
 * from a redundant `role` attribute, because the implicit role of `<nav>`,
 * `<main>` and `<aside>` is the one assistive technology will use.
 */

const IMPLICIT_ROLES: Record<string, string> = {
  NAV: 'navigation',
  MAIN: 'main',
  ASIDE: 'complementary',
  HEADER: 'banner',
  FOOTER: 'contentinfo',
  SECTION: 'region',
}

function roleOf(element: Element): string {
  return element.getAttribute('role') ?? IMPLICIT_ROLES[element.tagName] ?? element.tagName
}

function mountShell() {
  return mount(AppShell, {
    props: { sessionsTitle: 'Sessions', inspectorTitle: 'Inspector', controlsLabel: 'Panels' },
    slots: {
      sessions: '<button type="button">Session row</button>',
      default: '<p>Transcript</p>',
      inspector: '<button type="button">Approval row</button>',
    },
  })
}

describe('workspace landmarks', () => {
  it('gives the rail the navigation landmark the contract names', () => {
    const wrapper = mountShell()

    const rail = wrapper.get('[data-pane="sessions"]')
    expect(roleOf(rail.element)).toBe('navigation')
    expect(rail.attributes('aria-label')).toBe('Sessions')
  })

  it('gives the transcript column the main landmark', () => {
    const wrapper = mountShell()

    const transcript = wrapper.get('[data-pane="transcript"]')
    expect(roleOf(transcript.element)).toBe('main')
    expect(transcript.attributes('aria-label')).toBe('Run transcript')
  })

  it('gives the inspector the complementary landmark', () => {
    const wrapper = mountShell()

    const inspector = wrapper.get('[data-pane="inspector"]')
    expect(roleOf(inspector.element)).toBe('complementary')
    expect(inspector.attributes('aria-label')).toBe('Inspector')
  })

  it('keeps the mobile controls inside a navigation landmark too', () => {
    const wrapper = mountShell()

    // The controls replace the rail on a narrow viewport, so they answer to
    // the same landmark the rail does rather than being loose buttons.
    expect(roleOf(wrapper.get('[data-mobile-controls]').element)).toBe('navigation')
  })

  it('keeps the rail and the inspector outside the main landmark', () => {
    // The three columns are siblings: a rail or an inspector nested inside
    // `main` is content the reader never leaves the transcript to find.
    const wrapper = mountShell()

    const main = wrapper.get('[data-pane="transcript"]').element
    expect(main.contains(wrapper.get('[data-pane="sessions"]').element)).toBe(false)
    expect(main.contains(wrapper.get('[data-pane="inspector"]').element)).toBe(false)
  })

  it('gives the product header the banner landmark', () => {
    const wrapper = mount(WorkspaceHeader, {
      props: { connection: 'ready', workspaceName: 'Orchester' },
    })

    // The header is the shell's one banner: a second top-level header would
    // split the reader's "jump to the top" between two competing regions.
    expect(roleOf(wrapper.get('header').element)).toBe('banner')
  })

  it('keeps every routed view reachable through a main landmark', () => {
    // The shell no longer wraps the outlet in `main`, so each route has to
    // declare its own. A route that forgets leaves the reader with a page they
    // cannot name.
    const wrapper = mount(NotFoundView, {
      global: { stubs: { RouterLink: { template: '<a><slot /></a>' } } },
    })

    expect(roleOf(wrapper.get('[data-testid="not-found-view"]').element)).toBe('main')
  })
})
