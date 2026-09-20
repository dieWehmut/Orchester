/**
 * Settings search.
 *
 * Matching lives here rather than in the view so the panel order and the
 * searchable vocabulary stay in one list: a section that exists but cannot be
 * found by any word a reader would use is a section that may as well not exist.
 */

export type SettingsSectionId =
  | 'general'
  | 'appearance'
  | 'notifications'
  | 'import'
  | 'profile'
  | 'keybindings'
  | 'providers'
  | 'about'

export interface SettingsSectionEntry<Id extends string = string> {
  readonly id: Id
  readonly label: string
  readonly keywords: readonly string[]
}

/**
 * The searchable vocabulary, in the order the settings surface renders. The
 * labels are English fallbacks; the surface searches its own translated labels,
 * so the keywords carry the words that are not already on screen in either
 * language.
 */
export const SETTINGS_SECTIONS: readonly SettingsSectionEntry<SettingsSectionId>[] = [
  {
    id: 'general',
    label: 'General',
    keywords: ['language', 'locale', 'startup', 'terminal', 'bottom panel', 'inspector'],
  },
  {
    id: 'appearance',
    label: 'Appearance',
    keywords: ['theme', 'colour', 'color', 'font', 'radius', 'motion', 'density'],
  },
  { id: 'notifications', label: 'Notifications', keywords: ['alerts', 'toast', 'sound'] },
  { id: 'import', label: 'Import', keywords: ['profile', 'file', 'restore'] },
  { id: 'profile', label: 'Profile', keywords: ['account', 'identity', 'avatar'] },
  {
    id: 'keybindings',
    label: 'Keybindings',
    keywords: ['shortcut', 'keyboard', 'hotkey', 'binding', 'keys'],
  },
  { id: 'providers', label: 'Providers', keywords: ['model', 'api key', 'endpoint'] },
  { id: 'about', label: 'About', keywords: ['version', 'licence', 'license', 'build'] },
]

/**
 * An empty query is not a filter: it shows the whole list rather than none, so
 * clearing the box cannot leave the reader staring at an empty nav.
 */
export function filterSettingsSections<T extends SettingsSectionEntry>(
  sections: readonly T[],
  query: string,
): readonly T[] {
  const needle = query.trim().toLowerCase()
  if (needle.length === 0) return sections
  return sections.filter((section) => {
    const haystack = [section.label, ...section.keywords].map((value) => value.toLowerCase())
    return haystack.some((value) => value.includes(needle))
  })
}
