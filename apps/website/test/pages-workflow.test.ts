import { readFile } from 'node:fs/promises'
import { resolve } from 'node:path'

import { beforeAll, describe, expect, it } from 'vitest'

const workflowPath = resolve(process.cwd(), '../../.github/workflows/pages.yml')

let workflow = ''

beforeAll(async () => {
  workflow = await readFile(workflowPath, 'utf8')
})

describe('GitHub Pages workflow', () => {
  it('uses immutable revisions of official actions', () => {
    const actionUses = [...workflow.matchAll(/uses:\s+(actions\/[^@\s]+)@([0-9a-f]{40})/g)].map(
      ([, action, revision]) => [action, revision],
    )

    expect(actionUses).toEqual([
      ['actions/checkout', '9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0'],
      ['actions/setup-node', '249970729cb0ef3589644e2896645e5dc5ba9c38'],
      ['actions/configure-pages', '45bfe0192ca1faeb007ade9deae92b16b8254a0d'],
      ['actions/upload-pages-artifact', 'fc324d3547104276b827a68afc52ff2a11cc49c9'],
      ['actions/deploy-pages', 'cd2ce8fcbc39b97be8ca5fce6e763baed58fa128'],
    ])
    expect(workflow).not.toMatch(/uses:\s+actions\/[^@\s]+@v\d+/)
  })

  it('installs and verifies the website before uploading its base-path build', () => {
    expect(workflow).toContain('pnpm --dir apps install --frozen-lockfile')
    expect(workflow).toContain('pnpm --dir apps --filter @orchester/website typecheck')
    expect(workflow).toContain('pnpm --dir apps --filter @orchester/website test')
    expect(workflow).toContain('pnpm --dir apps --filter @orchester/website build')
    expect(workflow).toContain('BASE_PATH: /Orchester/')
    expect(workflow).toContain('VITE_GISCUS_REPO: ${{ vars.GISCUS_REPO }}')
    expect(workflow).toContain('VITE_GISCUS_REPO_ID: ${{ vars.GISCUS_REPO_ID }}')
    expect(workflow).toContain('VITE_GISCUS_CATEGORY: ${{ vars.GISCUS_CATEGORY }}')
    expect(workflow).toContain('VITE_GISCUS_CATEGORY_ID: ${{ vars.GISCUS_CATEGORY_ID }}')
    expect(workflow).toContain('path: apps/website/dist')
  })

  it('limits deployment to the Pages environment and cancels stale runs', () => {
    expect(workflow).toContain('cancel-in-progress: true')
    expect(workflow).toContain('pages: write')
    expect(workflow).toContain('id-token: write')
    expect(workflow).toContain('name: github-pages')
    expect(workflow).toContain('needs: build')
  })
})
