import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import PetCompanion from '../src/features/pet/PetCompanion.vue'
import { resetPetPackForTests } from '../src/features/pet/use-pet-pack'

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

    window.dispatchEvent(new MouseEvent('pointermove', { clientX: 500, clientY: 0 }))
    await flushPromises()

    expect(wrapper.get('[data-pet-companion]').attributes('data-pet-look')).toBeDefined()
    wrapper.unmount()
  })
})
