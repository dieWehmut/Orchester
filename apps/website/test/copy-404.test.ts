import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { afterEach, describe, expect, it } from 'vitest'

import { copyIndexTo404 } from '../src/build/copy-404'

const temporaryDirectories: string[] = []

afterEach(async () => {
  await Promise.all(
    temporaryDirectories.splice(0).map((directory) =>
      rm(directory, { force: true, recursive: true }),
    ),
  )
})

describe('copyIndexTo404', () => {
  it('copies the built index byte-for-byte for Pages deep links', async () => {
    const outDir = await mkdtemp(join(tmpdir(), 'orchester-pages-'))
    temporaryDirectories.push(outDir)
    const markup = '<!doctype html><title>Orchester</title>'
    await writeFile(join(outDir, 'index.html'), markup, 'utf8')

    await copyIndexTo404(outDir)

    await expect(readFile(join(outDir, '404.html'), 'utf8')).resolves.toBe(markup)
  })
})
