import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

function source(relativePath: string): string {
  return readFileSync(resolve(process.cwd(), 'src', relativePath), 'utf8')
}

describe('workspace visual policy', () => {
  it('does not use decorative gradients or viewport-scaled type in the empty workspace', () => {
    const mark = source('components/run/OrchesterMark.vue')
    const empty = source('components/run/EmptyWorkspace.vue')

    expect(mark).not.toMatch(/(?:linear|radial)-gradient\(/)
    expect(empty).not.toMatch(/\b(?:vw|vh|vmin|vmax)\b/)
  })
})
