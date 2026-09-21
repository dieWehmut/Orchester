import { describe, expect, it } from 'vitest'

import { giscusTermForRoute, parseGiscusConfig } from '../src/comments/giscus-config'

describe('Giscus configuration', () => {
  it('requires every public repository and category identifier', () => {
    expect(parseGiscusConfig({})).toBeNull()
    expect(
      parseGiscusConfig({
        VITE_GISCUS_REPO: 'dieWehmut/Orchester',
        VITE_GISCUS_REPO_ID: 'R_example',
        VITE_GISCUS_CATEGORY: 'Announcements',
      }),
    ).toBeNull()
  })

  it('normalizes a complete public configuration with safe defaults', () => {
    expect(
      parseGiscusConfig({
        VITE_GISCUS_REPO: ' dieWehmut/Orchester ',
        VITE_GISCUS_REPO_ID: ' R_example ',
        VITE_GISCUS_CATEGORY: ' Ideas ',
        VITE_GISCUS_CATEGORY_ID: ' DIC_example ',
      }),
    ).toEqual({
      repo: 'dieWehmut/Orchester',
      repoId: 'R_example',
      category: 'Ideas',
      categoryId: 'DIC_example',
      strict: '1',
      reactionsEnabled: '1',
      inputPosition: 'top',
      lang: 'en',
      loading: 'lazy',
    })
  })

  it('builds a stable discussion term from the route path', () => {
    expect(giscusTermForRoute('/')).toBe('orchester:home')
    expect(giscusTermForRoute('/architecture/')).toBe('orchester:architecture')
    expect(giscusTermForRoute('/Orchester/install?from=home#next')).toBe(
      'orchester:Orchester/install',
    )
  })
})
