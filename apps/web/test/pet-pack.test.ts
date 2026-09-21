import { describe, expect, it, vi } from 'vitest'

import {
  PET_PACK_URL,
  loadPetPack,
  resetPetPackForTests,
  usePetPack,
} from '../src/features/pet/use-pet-pack'

const MANIFEST = {
  id: 'xiaoxuan',
  displayName: '小萱',
  description: 'A poised silver-white bear-eared chibi companion.',
  spriteVersionNumber: 2,
  spritesheetPath: 'spritesheet.webp',
}

function response(body: unknown, status = 200): Response {
  return new Response(typeof body === 'string' ? body : JSON.stringify(body), { status })
}

describe('pet pack loading', () => {
  it('resolves the manifest into a servable pack and grid', async () => {
    const pack = await loadPetPack({ fetcher: vi.fn(async () => response(MANIFEST)) })

    expect(pack).toEqual({
      id: 'xiaoxuan',
      displayName: '小萱',
      description: MANIFEST.description,
      spritesheetUrl: '/pets/xiaoxuan/spritesheet.webp',
      grid: {
        frameWidth: 192,
        frameHeight: 208,
        columns: 8,
        rows: 11,
        frameCount: 88,
      },
    })
  })

  it('keeps the pack folder the app actually serves', () => {
    expect(PET_PACK_URL).toBe('/pets/xiaoxuan')
  })

  it('resolves to null instead of throwing when the pack is missing', async () => {
    expect(await loadPetPack({ fetcher: vi.fn(async () => response('nope', 404)) })).toBeNull()
    expect(await loadPetPack({ fetcher: vi.fn(async () => response({})) })).toBeNull()
  })

  it('survives a transport failure and a hostile manifest', async () => {
    const failing = vi.fn(async () => {
      throw new Error('offline')
    })
    expect(await loadPetPack({ fetcher: failing })).toBeNull()
    expect(
      await loadPetPack({ fetcher: vi.fn(async () => response('<html>not json</html>')) }),
    ).toBeNull()
  })

  it('shares one load between consumers', async () => {
    resetPetPackForTests()
    const fetcher = vi.fn(async () => response(MANIFEST))

    const first = usePetPack({ fetcher })
    const second = usePetPack({ fetcher })
    await vi.waitFor(() => expect(first.value).not.toBeNull())

    expect(second.value).toEqual(first.value)
    expect(fetcher).toHaveBeenCalledTimes(1)
  })
})
