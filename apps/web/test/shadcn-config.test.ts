import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

import { describe, expect, it } from 'vitest'

interface ShadcnConfig {
  style: string
  tailwind: { config: string; css: string; cssVariables: boolean }
  iconLibrary: string
  aliases: Record<string, string>
}

describe('shadcn-vue source configuration', () => {
  it('keeps generated components inside the WebUI and uses the shared CSS entry', () => {
    const config = JSON.parse(
      readFileSync(resolve(process.cwd(), 'components.json'), 'utf8'),
    ) as ShadcnConfig

    expect(config.style).toBe('new-york')
    expect(config.iconLibrary).toBe('lucide')
    expect(config.tailwind).toEqual({
      config: '',
      css: 'src/styles/tailwind.css',
      baseColor: 'neutral',
      cssVariables: true,
    })
    expect(config.aliases).toMatchObject({
      components: '@/components',
      ui: '@/components/ui',
      utils: '@/lib/utils',
    })
  })
})
