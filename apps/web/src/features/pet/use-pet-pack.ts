/**
 * Load the bundled pet pack once and hand back a ready-to-draw grid.
 *
 * The pack is fixed: the desktop shell ships one companion, so there is no
 * picker and no persisted selection to reconcile. A missing or malformed pack
 * resolves to null rather than throwing, because a decoration must never be the
 * reason the workspace fails to render.
 */

import { readonly, ref, type Ref } from 'vue'

import { parsePetManifest, petGridFor, resolveSpritesheetUrl, type PetGrid } from './pet-manifest'

export interface PetPack {
  readonly id: string
  readonly displayName: string
  readonly description: string
  readonly spritesheetUrl: string
  readonly grid: PetGrid
}

/** Where the fixed companion lives inside the served bundle. */
export const PET_PACK_URL = '/pets/xiaoxuan'

const pack = ref<PetPack | null>(null)
let loading: Promise<PetPack | null> | null = null

export interface LoadPetPackOptions {
  /** Injected for tests; defaults to the global fetch. */
  readonly fetcher?: typeof fetch
  readonly packUrl?: string
}

export async function loadPetPack(options: LoadPetPackOptions = {}): Promise<PetPack | null> {
  const packUrl = options.packUrl ?? PET_PACK_URL
  const fetcher = options.fetcher ?? globalThis.fetch
  if (typeof fetcher !== 'function') return null
  try {
    const response = await fetcher(`${packUrl}/pet.json`)
    if (!response.ok) return null
    const manifest = parsePetManifest(await response.json())
    if (!manifest) return null
    return {
      id: manifest.id,
      displayName: manifest.displayName,
      description: manifest.description,
      spritesheetUrl: resolveSpritesheetUrl(packUrl, manifest.spritesheetPath),
      grid: petGridFor(manifest),
    }
  } catch {
    return null
  }
}

/** The resolved pack, loaded on first use and shared by every consumer. */
export function usePetPack(options: LoadPetPackOptions = {}): Ref<PetPack | null> {
  if (!loading && pack.value === null) {
    loading = loadPetPack(options).then((resolved) => {
      pack.value = resolved
      loading = null
      return resolved
    })
  }
  return readonly(pack) as Ref<PetPack | null>
}

/** Drop the cached pack. Exists for tests, which need a clean module. */
export function resetPetPackForTests(): void {
  pack.value = null
  loading = null
}
