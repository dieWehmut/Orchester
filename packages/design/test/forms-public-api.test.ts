import { describe, expect, it } from 'vitest'

import {
  AppCheckbox,
  AppField,
  AppInput,
  AppSelect,
  AppSwitch,
  AppTextarea,
  type AppSelectOption,
} from '../src'

describe('form public API', () => {
  it('exports every form primitive and the shared option contract', () => {
    const option: AppSelectOption = { value: 'codex', label: 'Codex' }

    expect([AppCheckbox, AppField, AppInput, AppSelect, AppSwitch, AppTextarea]).not.toContain(
      undefined,
    )
    expect(option.value).toBe('codex')
  })
})
