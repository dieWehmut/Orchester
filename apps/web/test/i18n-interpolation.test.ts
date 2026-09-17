import { describe, expect, it } from 'vitest'

import { createI18n } from '../src/i18n'

describe('WebUI locale interpolation', () => {
  it('replaces named placeholders in a message', () => {
    const i18n = createI18n('en')

    expect(i18n.t('workspace.greeting', { name: 'Orchester' })).toBe(
      'What should we build in Orchester?',
    )
  })

  it('replaces placeholders after switching locale', () => {
    const i18n = createI18n('zh-CN')

    expect(i18n.t('workspace.greeting', { name: 'Orchester' })).toBe(
      '你想让我们在 Orchester 中构建什么？',
    )
  })

  it('leaves unknown placeholders untouched and keeps messages without params stable', () => {
    const i18n = createI18n('en')

    expect(i18n.t('workspace.greeting', { other: 'x' })).toBe('What should we build in {name}?')
    expect(i18n.t('app.name')).toBe('Orchester')
  })
})
