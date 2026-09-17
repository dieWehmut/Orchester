import { describe, expect, it } from 'vitest'

import { architectureBoundary, architectureStages } from '../src/content/architecture'
import { homeAdapters, homeCapabilities, governancePrinciples } from '../src/content/home'
import { installPrerequisites, installSteps } from '../src/content/install'

describe('typed website content', () => {
  it('keeps the home content collections complete and keyed', () => {
    expect(homeCapabilities).toHaveLength(3)
    expect(homeAdapters).toHaveLength(3)
    expect(governancePrinciples).toHaveLength(3)
    expect(new Set(homeCapabilities.map((item) => item.id)).size).toBe(3)
    expect(new Set(homeAdapters.map((item) => item.id)).size).toBe(3)
  })

  it('describes a four-stage runtime flow and an explicit boundary', () => {
    expect(architectureStages).toHaveLength(4)
    expect(architectureStages.map((stage) => stage.id)).toEqual([
      'browser',
      'service',
      'runtime',
      'provider',
    ])
    expect(architectureBoundary.title).toContain('Boundary')
  })

  it('keeps install commands deterministic and prerequisites explicit', () => {
    expect(installSteps).toHaveLength(3)
    expect(installSteps.every((step) => step.command.length > 0)).toBe(true)
    expect(installPrerequisites).toContain('Node.js 22 or newer')
  })
})
