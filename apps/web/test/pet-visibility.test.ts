import { beforeEach, describe, expect, it } from 'vitest'

import {
  PET_VISIBILITY_STORAGE_KEY,
  resetPetVisibilityForTests,
  usePetVisibility,
} from '../src/features/pet'

describe('pet visibility', () => {
  beforeEach(() => {
    resetPetVisibilityForTests()
    localStorage.clear()
  })

  it('shows the companion by default', () => {
    const { visible, hidden } = usePetVisibility()

    expect(visible.value).toBe(true)
    expect(hidden.value).toBe(false)
  })

  it('remembers that the companion was hidden', () => {
    usePetVisibility().hide()

    expect(localStorage.getItem(PET_VISIBILITY_STORAGE_KEY)).toBe('false')
    resetPetVisibilityForTests()
    expect(usePetVisibility().visible.value).toBe(false)
  })

  it('toggles back to visible and persists it', () => {
    const api = usePetVisibility()
    api.hide()
    api.toggle()

    expect(api.visible.value).toBe(true)
    expect(localStorage.getItem(PET_VISIBILITY_STORAGE_KEY)).toBe('true')
  })
})
