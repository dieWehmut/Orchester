import { describe, expect, it } from 'vitest'

import { cn } from '../src/lib/utils'

describe('cn class utility', () => {
  it('merges conflicting Tailwind utilities with the last intent winning', () => {
    expect(cn('px-2 text-sm', 'px-4', false && 'hidden')).toBe('text-sm px-4')
  })

  it('preserves arbitrary values and conditional class names', () => {
    expect(cn('bg-[color:var(--color-accent)]', { 'opacity-50': true })).toBe(
      'bg-[color:var(--color-accent)] opacity-50',
    )
  })
})
