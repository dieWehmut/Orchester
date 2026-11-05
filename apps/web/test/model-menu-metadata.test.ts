import { describe, expect, it } from 'vitest'
import { parseModelCatalog } from '@orchester/protokoll'
import { modelMenuGroups, modelReasoningNames, REASONING_EFFORTS } from '../src/components/run/model-menu'
import { MODEL_CATALOG_FIXTURE } from './fixtures/model-catalog'

describe('configured model metadata', () => {
  it('uses display names while keeping API IDs and limits visible', () => {
    const catalog = structuredClone(MODEL_CATALOG_FIXTURE)
    Object.assign(catalog.profiles[0]!, { display_name: 'Review assistant', context_window: 1000000, max_output_tokens: 256000 })
    const row = modelMenuGroups(catalog)[0]!.choices.find(row => row.id === 'profile:review')!
    expect(row.label).toBe('Review assistant')
    expect(row.description).toContain('gpt-5.6')
    expect(row.description).toContain('1,000,000')
  })
  it('distinguishes inherited, disabled and custom reasoning menus', () => {
    const catalog = structuredClone(MODEL_CATALOG_FIXTURE)
    if (catalog.active.state !== 'configured') throw new Error('fixture')
    expect(modelReasoningNames(catalog)).toEqual(REASONING_EFFORTS)
    catalog.active.choice.reasoning_levels = []
    expect(modelReasoningNames(catalog)).toEqual([])
    catalog.active.choice.reasoning_levels = [{ level: 'careful', api_value: 'high' }, { level: 'off', api_value: null }]
    expect(modelReasoningNames(catalog)).toEqual(['careful', 'off'])
    expect(parseModelCatalog(catalog)?.active).toEqual(catalog.active)
  })
  it('rejects malformed metadata from the network', () => {
    const catalog = structuredClone(MODEL_CATALOG_FIXTURE)
    Object.assign(catalog.profiles[0]!, { context_window: -1 })
    expect(parseModelCatalog(catalog)).toBeNull()
  })
})
