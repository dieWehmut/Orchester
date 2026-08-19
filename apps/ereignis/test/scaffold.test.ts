import { describe, expect, it } from 'vitest'

describe('@orchester/ereignis', () => {
  it('loads its source entrypoint', async () => {
    expect(Object.keys(await import('../src/index'))).toEqual([])
  })
})
