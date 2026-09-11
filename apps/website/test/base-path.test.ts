import { describe, expect, it } from 'vitest'

import { normalizeBasePath } from '../src/base-path'

describe('normalizeBasePath', () => {
  it.each([
    [undefined, '/'],
    ['', '/'],
    ['   ', '/'],
    ['/', '/'],
    ['Orchester', '/Orchester/'],
    ['/Orchester', '/Orchester/'],
    ['/Orchester/', '/Orchester/'],
    ['///Orchester///', '/Orchester/'],
  ])('normalizes %s to %s', (value, expected) => {
    expect(normalizeBasePath(value)).toBe(expected)
  })
})
