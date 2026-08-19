import { describe, expect, it } from 'vitest'

import { createI18n, supportedLocales } from '../src/i18n'

describe('WebUI locale service', () => {
  it('exposes all supported locales and stable route labels', () => {
    expect(supportedLocales).toEqual(['en', 'zh-CN', 'zh-TW'])

    const i18n = createI18n('zh-CN')
    expect(i18n.locale.value).toBe('zh-CN')
    expect(i18n.t('routes.workspace')).toBe('工作区')
  })

  it('falls back to English when a requested locale is unsupported', () => {
    const i18n = createI18n('fr')

    expect(i18n.locale.value).toBe('en')
    expect(i18n.t('routes.settings')).toBe('Settings')
  })

  it('keeps the complete message key set available in every locale', () => {
    for (const locale of supportedLocales) {
      const i18n = createI18n(locale)
      expect(i18n.t('workspace.description')).not.toBe('workspace.description')
      expect(i18n.t('settings.description')).not.toBe('settings.description')
      expect(i18n.t('notFound.return')).not.toBe('notFound.return')
    }
  })
})
