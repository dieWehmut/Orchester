/**
 * Whether the ambient companion is drawn.
 *
 * A single shared flag rather than a store: it is one boolean on one shell, and
 * the shell already owns the only thing that changes it. Persisting it keeps the
 * companion from reappearing every reload after the user hid it, and the
 * read/write pair is wrapped so a blocked localStorage cannot break the shell.
 */

import { computed, ref, type ComputedRef, type Ref } from 'vue'

import { readStored, writeStored } from '@orchester/design'

export const PET_VISIBILITY_STORAGE_KEY = 'orchester.pet.visible'

const visible = ref(true)
let initialized = false

function init(): void {
  const stored = readStored(PET_VISIBILITY_STORAGE_KEY)
  visible.value = stored === null ? true : stored === 'true'
  initialized = true
}

export interface PetVisibilityApi {
  visible: Readonly<Ref<boolean>>
  hidden: ComputedRef<boolean>
  show: () => void
  hide: () => void
  toggle: () => void
}

export function usePetVisibility(): PetVisibilityApi {
  if (!initialized) init()
  return {
    visible: visible,
    hidden: computed(() => !visible.value),
    show: () => {
      visible.value = true
      writeStored(PET_VISIBILITY_STORAGE_KEY, 'true')
    },
    hide: () => {
      visible.value = false
      writeStored(PET_VISIBILITY_STORAGE_KEY, 'false')
    },
    toggle: () => {
      visible.value = !visible.value
      writeStored(PET_VISIBILITY_STORAGE_KEY, String(visible.value))
    },
  }
}

/** Reset the module singleton. Exists for tests, which need a clean module. */
export function resetPetVisibilityForTests(): void {
  initialized = false
  visible.value = true
}
