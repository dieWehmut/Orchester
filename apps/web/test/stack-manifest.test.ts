import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

import { describe, expect, it } from 'vitest'

interface PackageManifest {
  dependencies?: Record<string, string>
  devDependencies?: Record<string, string>
}

const manifestPath = resolve(process.cwd(), 'package.json')
const manifest = JSON.parse(readFileSync(manifestPath, 'utf8')) as PackageManifest

describe('WebUI stack manifest', () => {
  it('pins the requested Vue application and shadcn-vue runtime foundations', () => {
    expect(manifest.dependencies).toMatchObject({
      '@lucide/vue': '^1.32.0',
      '@vue/devtools-api': '^8.1.5',
      pinia: '^4.0.3',
      'class-variance-authority': '^0.7.1',
      clsx: '^2.1.1',
      'reka-ui': '^2.10.3',
      'tailwind-merge': '^3.6.0',
    })
  })

  it('pins Tailwind and its Vite integration as build-time dependencies', () => {
    expect(manifest.devDependencies).toMatchObject({
      '@tailwindcss/vite': '^4.3.3',
      tailwindcss: '^4.3.3',
    })
  })
})
