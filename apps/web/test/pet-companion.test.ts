import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import PetCompanion from '../src/features/pet/PetCompanion.vue'
import { resetPetPackForTests } from '../src/features/pet/use-pet-pack'
import { PET_ROAM_FIRST_PAUSE_MS, PET_ROAM_STRIDE_PX } from '../src/features/pet/pet-roam'
import { petPoseHoldMs, petWalkCycleMs } from '../src/features/pet/pet-animations'

const MANIFEST = {
  id: 'xiaoxuan',
  displayName: '小萱',
  description: 'A poised silver-white bear-eared chibi companion.',
  spriteVersionNumber: 2,
  spritesheetPath: 'spritesheet.webp',
}

function stubPackFetch(): void {
  vi.stubGlobal(
    'fetch',
    vi.fn(async () => new Response(JSON.stringify(MANIFEST), { status: 200 })),
  )
}

beforeEach(() => {
  resetPetPackForTests()
  stubPackFetch()
})

afterEach(() => {
  vi.useRealTimers()
})

describe('PetCompanion', () => {
  it('draws the sprite cell from the loaded pack', async () => {
    const wrapper = mount(PetCompanion, { props: { label: 'Companion' } })
    await flushPromises()

    const sprite = wrapper.get('[data-pet-sprite]')
    const style = sprite.attributes('style') ?? ''

    expect(style).toContain('/pets/xiaoxuan/spritesheet.webp')
    expect(style).toContain('background-size: 800% 1100%')
    expect(wrapper.get('[data-pet-companion]').attributes('data-pet-animation')).toBe('idle')
  })

  it('reports the pose as a decorative element when unlabelled', async () => {
    const wrapper = mount(PetCompanion)
    await flushPromises()

    const root = wrapper.get('[data-pet-companion]')
    expect(root.attributes('role')).toBe('presentation')
    expect(root.attributes('aria-hidden')).toBe('true')
  })

  it('exposes a labelled image while a notification is showing', async () => {
    const wrapper = mount(PetCompanion, { props: { label: 'Running' } })
    await flushPromises()

    const root = wrapper.get('[data-pet-companion]')
    expect(root.attributes('role')).toBe('img')
    expect(root.attributes('aria-label')).toBe('Running')
    expect(root.attributes('aria-hidden')).toBeUndefined()
  })

  it('follows the requested animation track', async () => {
    const wrapper = mount(PetCompanion, { props: { animation: 'failed' } })
    await flushPromises()

    expect(wrapper.get('[data-pet-companion]').attributes('data-pet-animation')).toBe('failed')
    expect(Number(wrapper.get('[data-pet-companion]').attributes('data-pet-frame'))).toBe(40)
  })

  it('freezes on the first frame under reduced motion', async () => {
    const wrapper = mount(PetCompanion, {
      props: { animation: 'running', reducedMotion: true },
    })
    await flushPromises()

    expect(wrapper.get('[data-pet-companion]').attributes('data-pet-frame')).toBe('56')
  })

  it('renders nothing but stays mounted when the pack cannot be loaded', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => new Response('missing', { status: 404 })),
    )
    resetPetPackForTests()

    const wrapper = mount(PetCompanion)
    await flushPromises()

    expect(wrapper.find('[data-pet-sprite]').attributes('style')).toBeUndefined()
    expect(wrapper.get('[data-pet-companion]')).toBeTruthy()
  })

  it('maps a pointer offset onto a look frame', async () => {
    const wrapper = mount(PetCompanion, {
      props: { label: 'Companion', trackPointer: true },
      attachTo: document.body,
    })
    await flushPromises()

    // The pointer has to come near the companion to be addressing it: a
    // pointer working elsewhere on the page is not a look it should hold.
    window.dispatchEvent(new MouseEvent('pointermove', { clientX: 60, clientY: 0 }))
    await flushPromises()

    expect(wrapper.get('[data-pet-companion]').attributes('data-pet-look')).toBeDefined()
    wrapper.unmount()
  })

  it('ignores a pointer that is only on the same screen', async () => {
    const wrapper = mount(PetCompanion, {
      props: { label: 'Companion', trackPointer: true },
      attachTo: document.body,
    })
    await flushPromises()

    window.dispatchEvent(new MouseEvent('pointermove', { clientX: 5000, clientY: 5000 }))
    await flushPromises()

    expect(wrapper.get('[data-pet-companion]').attributes('data-pet-look')).toBeUndefined()
    wrapper.unmount()
  })
})

/**
 * The roam, as the reference companion does it: while the run has nothing to
 * say the companion pads along the stage its host opened, and the moment the
 * run asks for a pose it stops where it stands instead of finishing the walk.
 */
describe('PetCompanion roam', () => {
  function mountRoaming(span = 400) {
    return mount(PetCompanion, {
      props: { roam: true, roamSpan: span, trackPointer: false },
    })
  }

  it('stands still until the stage has room for a stride', async () => {
    vi.useFakeTimers()
    const wrapper = mountRoaming(PET_ROAM_STRIDE_PX - 1)
    await flushPromises()

    vi.advanceTimersByTime(PET_ROAM_FIRST_PAUSE_MS * 4)
    await flushPromises()

    expect(wrapper.get('[data-pet-companion]').attributes('data-pet-roam')).toBe('resting')
    wrapper.unmount()
  })

  it('walks a leg in the row that faces the way it goes', async () => {
    vi.useFakeTimers()
    const wrapper = mountRoaming()
    await flushPromises()

    expect(wrapper.get('[data-pet-companion]').attributes('data-pet-roam')).toBe('resting')

    vi.advanceTimersByTime(PET_ROAM_FIRST_PAUSE_MS)
    await flushPromises()

    const companion = wrapper.get('[data-pet-companion]')
    expect(companion.attributes('data-pet-roam')).toBe('walking')
    const row = companion.attributes('data-pet-animation')
    expect(['running-left', 'running-right']).toContain(row)
    expect(companion.attributes('data-pet-position')).not.toBe('0')
    wrapper.unmount()
  })

  it('times the glide to the leg it is walking', async () => {
    vi.useFakeTimers()
    const wrapper = mountRoaming()
    await flushPromises()
    vi.advanceTimersByTime(PET_ROAM_FIRST_PAUSE_MS)
    await flushPromises()

    const companion = wrapper.get('[data-pet-companion]')
    const position = Number(companion.attributes('data-pet-position'))
    const sprite = wrapper.get('[data-pet-sprite]')

    expect(sprite.attributes('style')).toContain('transition-duration:')
    expect(position % PET_ROAM_STRIDE_PX).toBe(0)
    wrapper.unmount()
  })

  it('stops where it stands when the run asks for a pose', async () => {
    vi.useFakeTimers()
    const wrapper = mountRoaming()
    await flushPromises()
    vi.advanceTimersByTime(PET_ROAM_FIRST_PAUSE_MS)
    await flushPromises()
    const walking = Number(wrapper.get('[data-pet-companion]').attributes('data-pet-position'))

    // Halfway through the leg the run turns busy, so the walk is interrupted
    // and the companion is left partway between where it stood and its goal.
    vi.advanceTimersByTime(Math.floor(petWalkCycleMs() / 2))
    await wrapper.setProps({ animation: 'running' })
    await flushPromises()

    const companion = wrapper.get('[data-pet-companion]')
    expect(companion.attributes('data-pet-roam')).toBe('resting')
    expect(companion.attributes('data-pet-animation')).toBe('running')
    const stopped = Number(companion.attributes('data-pet-position'))
    expect(Math.abs(stopped)).toBeLessThan(Math.abs(walking))
    wrapper.unmount()
  })

  it('keeps the companion where it hid when motion is reduced', async () => {
    vi.useFakeTimers()
    const wrapper = mountRoaming()
    await flushPromises()
    vi.advanceTimersByTime(PET_ROAM_FIRST_PAUSE_MS)
    await flushPromises()
    vi.advanceTimersByTime(Math.floor(petWalkCycleMs() / 2))

    await wrapper.setProps({ reducedMotion: true })
    await flushPromises()
    const stopped = Number(wrapper.get('[data-pet-companion]').attributes('data-pet-position'))
    expect(stopped).toBeGreaterThan(0)

    vi.advanceTimersByTime(PET_ROAM_FIRST_PAUSE_MS * 4)
    await flushPromises()

    const companion = wrapper.get('[data-pet-companion]')
    expect(companion.attributes('data-pet-roam')).toBe('resting')
    expect(Number(companion.attributes('data-pet-position'))).toBe(stopped)
    wrapper.unmount()
  })

  it('lets a settled run finish its review before it starts padding about', async () => {
    vi.useFakeTimers()
    const wrapper = mount(PetCompanion, {
      props: { animation: 'review', roam: true, roamSpan: 400, trackPointer: false },
    })
    await flushPromises()
    const hold = petPoseHoldMs('review')
    expect(hold).toBeGreaterThan(PET_ROAM_FIRST_PAUSE_MS)

    // The companion's own beat would already be up, but the review still has
    // frames to play, so the walk waits for them.
    vi.advanceTimersByTime(PET_ROAM_FIRST_PAUSE_MS + 1)
    await flushPromises()
    expect(wrapper.get('[data-pet-companion]').attributes('data-pet-roam')).toBe('resting')

    vi.advanceTimersByTime(hold)
    await flushPromises()
    expect(wrapper.get('[data-pet-companion]').attributes('data-pet-roam')).toBe('walking')
    wrapper.unmount()
  })

  it('stops the walk under a pointer look, and picks the beat up again', async () => {
    vi.useFakeTimers()
    const wrapper = mount(PetCompanion, {
      props: { roam: true, roamSpan: 400, trackPointer: true },
      attachTo: document.body,
    })
    await flushPromises()
    vi.advanceTimersByTime(PET_ROAM_FIRST_PAUSE_MS)
    await flushPromises()
    expect(wrapper.get('[data-pet-companion]').attributes('data-pet-roam')).toBe('walking')

    // The pointer arrives: the companion attends to it and stops walking where
    // it stood, which is a look rather than a leg.
    window.dispatchEvent(new MouseEvent('pointermove', { clientX: 60, clientY: 0 }))
    await flushPromises()
    const companion = wrapper.get('[data-pet-companion]')
    expect(companion.attributes('data-pet-look')).toBeDefined()
    expect(companion.attributes('data-pet-roam')).toBe('resting')

    // The pointer leaves, so the companion goes back to its own habits.
    window.dispatchEvent(new MouseEvent('pointermove', { clientX: 5000, clientY: 5000 }))
    await flushPromises()
    vi.advanceTimersByTime(PET_ROAM_FIRST_PAUSE_MS)
    await flushPromises()
    expect(wrapper.get('[data-pet-companion]').attributes('data-pet-roam')).toBe('walking')
    wrapper.unmount()
  })
})
