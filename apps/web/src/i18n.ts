import { computed, inject, ref, type App, type InjectionKey, type Ref } from 'vue'

import en from './locales/en.json'
import zhCN from './locales/zh-CN.json'
import zhTW from './locales/zh-TW.json'

export const supportedLocales = ['en', 'zh-CN', 'zh-TW'] as const
export type Locale = (typeof supportedLocales)[number]
interface MessageTree {
  [key: string]: string | MessageTree
}
type NestedKey<T> = {
  [Key in keyof T & string]: T[Key] extends string
    ? Key
    : T[Key] extends Record<string, unknown>
      ? `${Key}.${NestedKey<T[Key]>}`
      : never
}[keyof T & string]
export type MessageKey = NestedKey<typeof en>

const messages = { en, 'zh-CN': zhCN, 'zh-TW': zhTW } satisfies Record<Locale, typeof en>

function normalizeLocale(locale: string | undefined): Locale {
  if (locale && supportedLocales.includes(locale as Locale)) return locale as Locale
  if (locale?.toLowerCase().startsWith('zh-tw')) return 'zh-TW'
  if (locale?.toLowerCase().startsWith('zh')) return 'zh-CN'
  return 'en'
}

function readMessage(tree: MessageTree, key: MessageKey): string {
  const value = key.split('.').reduce<string | MessageTree | undefined>((current, part) => {
    if (!current || typeof current === 'string') return undefined
    return current[part]
  }, tree)
  return typeof value === 'string' ? value : key
}

export interface I18n {
  locale: Ref<Locale>
  t: (key: MessageKey) => string
  setLocale: (locale: string) => void
  install: (app: App) => void
}

const I18N_KEY: InjectionKey<I18n> = Symbol('orchester-i18n')

export function createI18n(initialLocale?: string): I18n {
  const locale = ref<Locale>(normalizeLocale(initialLocale ?? 'en'))
  const t = (key: MessageKey): string => readMessage(messages[locale.value], key)
  const setLocale = (next: string): void => {
    locale.value = normalizeLocale(next)
  }
  const install = (app: App): void => {
    app.provide(I18N_KEY, { locale: computed(() => locale.value), t, setLocale, install })
  }

  return { locale, t, setLocale, install }
}

export const appI18n = createI18n(typeof navigator === 'undefined' ? 'en' : navigator.language)

export function useI18n(): I18n {
  return inject(I18N_KEY, appI18n)
}
