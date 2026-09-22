<script setup lang="ts">
/**
 * The settings surface.
 *
 * Laid out like the reference: a titled section list on the left, and on the
 * right the appearance screen — the theme cards, a live code preview, then one
 * card per theme carrying the hue, background and foreground that theme
 * resolves to, and finally a table of the axes that are not about a theme.
 */
import {
  Bell,
  Compass,
  Copy,
  Download,
  Info,
  ArrowLeft,
  MonitorSmartphone,
  Moon,
  Palette,
  PawPrint,
  Keyboard,
  Plug,
  RotateCcw,
  Search,
  Settings2,
  Upload,
  UserRound,
} from '@lucide/vue'
import {
  AppBadge,
  AppButton,
  AppSegmentedControl,
  AppSelect,
  AppSwitch,
  CodePreview,
  EmptyState,
  ThemePreviewCard,
  exportAppearanceProfile,
  importAppearanceProfile,
  initAppearance,
  resetAppearance,
  useAppearance,
  type AppearanceApi,
  type AppSegmentOption,
  type ColorScheme,
  type ThemeMode,
  type ThemePreference,
} from '@orchester/design'
import { computed, inject, ref } from 'vue'
import { routerKey } from 'vue-router'

import { usePetVisibility } from '../features/pet'
import {
  themeAccent,
  themeAccentContrast,
  themeClipboardPayload,
  themeColours,
} from '../components/settings/theme-colours'
import { useI18n } from '../i18n'
import ShortcutEditor from '../components/settings/ShortcutEditor.vue'
import { readDocumentPlatform, readSystemPlatform } from '@orchester/design'
import { shortcutRegistry } from '../shortcuts'
import {
  readTerminalPlacement,
  writeTerminalPlacement,
  type TerminalPlacement,
} from '../components/layout/terminal-placement'
import {
  filterSettingsSections,
  SETTINGS_SECTIONS,
  type SettingsSectionEntry,
  type SettingsSectionId,
} from '../components/settings/settings-search'

type SettingsSection =
  | 'general'
  | 'notifications'
  | 'import'
  | 'profile'
  | 'appearance'
  | 'pet'
  | 'keybindings'
  | 'providers'
  | 'about'

const { t, locale, setLocale } = useI18n()

/**
 * The way back, section 4.8.
 *
 * Settings is a route rather than an overlay, so the reference's
 * "return to app" row is the browser's own history when there is one and the
 * workspace route when the reader landed here directly. Reading the router off
 * the injection key rather than importing the singleton keeps the view
 * mountable in a test that never installed one.
 */
const router = inject(routerKey, null)

function returnToWorkspace(): void {
  void router?.push({ name: 'workspace' })
}

const importTrigger = ref<HTMLInputElement | null>(null)

initAppearance()
const appearance: AppearanceApi = useAppearance()
const petVisibility = usePetVisibility()

const petVisible = computed({
  get: () => petVisibility.visible.value,
  set: (value: boolean) => (value ? petVisibility.show() : petVisibility.hide()),
})

const activeSection = ref<SettingsSection>('appearance')

interface SettingsNavEntry {
  id: SettingsSection
  labelKey: Parameters<typeof t>[0]
  icon: typeof Info
  group: 'personal' | 'integrations'
}

const navEntries: readonly SettingsNavEntry[] = [
  { id: 'general', labelKey: 'settings.sections.general', icon: Settings2, group: 'personal' },
  { id: 'notifications', labelKey: 'settings.sections.notifications', icon: Bell, group: 'personal' },
  { id: 'import', labelKey: 'settings.sections.import', icon: Download, group: 'personal' },
  { id: 'profile', labelKey: 'settings.sections.profile', icon: UserRound, group: 'personal' },
  { id: 'appearance', labelKey: 'settings.sections.appearance', icon: Palette, group: 'personal' },
  { id: 'pet', labelKey: 'pet.title', icon: PawPrint, group: 'personal' },
  {
    id: 'keybindings',
    labelKey: 'settings.sections.keybindings',
    icon: Keyboard,
    group: 'personal',
  },
  { id: 'providers', labelKey: 'settings.sections.providers', icon: Plug, group: 'integrations' },
  { id: 'about', labelKey: 'settings.sections.about', icon: Info, group: 'integrations' },
]

/**
 * Search across the nav.
 *
 * The query is matched against the section's translated name as well as the
 * vocabulary the search module carries, in that section's order. Filtering the
 * nav rather than the panels keeps a match visible as a destination: a reader
 * searching for "fonts" still lands on the appearance screen they know, with
 * its own controls, instead of on a list of loose rows.
 */
const settingsQuery = ref('')

const filteredSections = computed<readonly SettingsNavEntry[]>(() => {
  const searchable: readonly (SettingsNavEntry & SettingsSectionEntry)[] = navEntries.map(
    (entry) => {
      const vocabulary = SETTINGS_SECTIONS.find((section) => section.id === (entry.id as SettingsSectionId))
      return {
        ...entry,
        label: t(entry.labelKey),
        keywords: vocabulary?.keywords ?? [],
      }
    },
  )
  const matched = filterSettingsSections(searchable, settingsQuery.value)
  const matchedIds = new Set(matched.map((entry) => entry.id))
  return navEntries.filter((entry) => matchedIds.has(entry.id))
})

const generalSections = computed(() =>
  filteredSections.value.filter((entry) => entry.group === 'personal'),
)

const integrationSections = computed(() =>
  filteredSections.value.filter((entry) => entry.group === 'integrations'),
)

const localeOptions = computed(() => [
  { value: 'en', label: t('settings.locale.en') },
  { value: 'zh-CN', label: t('settings.locale.zhCN') },
  { value: 'zh-TW', label: t('settings.locale.zhTW') },
])

const localeValue = computed({
  get: () => locale.value,
  set: (value: string) => setLocale(value),
})

const themeCards = computed<
  { value: ThemePreference; label: string; icon: typeof MonitorSmartphone }[]
>(() => [
  { value: 'system', label: t('settings.appearance.system'), icon: MonitorSmartphone },
  { value: 'light', label: t('settings.appearance.light'), icon: Compass },
  { value: 'dark', label: t('settings.appearance.dark'), icon: Moon },
])

/**
 * The two themes, in the order the reference draws them.
 *
 * Not `THEME_PREFERENCES`: "system" is a preference rather than a palette, and
 * it is the cards above that choose it. These two are the columns the reader
 * sets a colour for.
 */
const themeModes = ['light', 'dark'] as const

const schemeOptions = computed(() => [
  { value: 'rose', label: t('settings.colorScheme.rose') },
  { value: 'codex', label: t('settings.colorScheme.codex') },
  { value: 'violet', label: t('settings.colorScheme.violet') },
  { value: 'teal', label: t('settings.colorScheme.teal') },
])

function themeLabel(mode: ThemeMode): string {
  return mode === 'light'
    ? t('settings.appearance.lightTheme')
    : t('settings.appearance.darkTheme')
}

/** The hue one theme is set in. */
function schemeFor(mode: ThemeMode): ColorScheme {
  return appearance.colorSchemes.value[mode]
}

/** What one theme's card reports, in that theme rather than the active one. */
function coloursFor(mode: ThemeMode) {
  return themeColours(mode)
}

function accentFor(mode: ThemeMode): string {
  return themeAccent(mode, schemeFor(mode))
}

/** The text colour that rides on that accent, for the card's own theme. */
function accentContrastFor(mode: ThemeMode): string {
  return themeAccentContrast(mode, schemeFor(mode))
}

/**
 * Copy one theme as text, the way the reference's "copy theme" does.
 *
 * The card is already printing these values, so what lands on the clipboard is
 * the same four facts rather than a screenshot of them.
 */
async function copyTheme(mode: ThemeMode): Promise<void> {
  const text = themeClipboardPayload(mode, schemeFor(mode))
  try {
    await navigator.clipboard?.writeText(text)
  } catch {
    /* A clipboard the browser refuses is not worth a broken screen. */
  }
}

const uiFontOptions = computed(() => [
  { value: 'system', label: t('settings.uiFont.system') },
  { value: 'sans', label: t('settings.uiFont.sans') },
  { value: 'serif', label: t('settings.uiFont.serif') },
])

const uiFontValue = computed({
  get: () => appearance.uiFont.value,
  set: (value: string) => appearance.setUiFont(value as never),
})

const fontWeightOptions = computed(() => [
  { value: 'regular', label: t('settings.fontWeight.regular') },
  { value: 'medium', label: t('settings.fontWeight.medium') },
])

const uiFontWeightValue = computed({
  get: () => appearance.uiFontWeight.value,
  set: (value: string) => appearance.setUiFontWeight(value as never),
})

const contentFontWeightValue = computed({
  get: () => appearance.contentFontWeight.value,
  set: (value: string) => appearance.setContentFontWeight(value as never),
})

const contentFontOptions = computed(() => [
  { value: 'ui', label: t('settings.contentFont.ui') },
  { value: 'mono', label: t('settings.contentFont.mono') },
])

const contentFontValue = computed({
  get: () => appearance.contentFont.value,
  set: (value: string) => appearance.setContentFont(value as never),
})

/**
 * Translucency needs a window behind it to see through, so the switch is only
 * offered where the shell can honour it. Elsewhere the row still explains what
 * the setting is and says why it is off rather than pretending it applied.
 */
const supportsTranslucency = computed(() => appearance.surface.value === 'desktop')

/**
 * The platform the editor names its modifier for. The document attribute is
 * the shell's own answer, and the user agent is the fallback for a surface
 * that has not set it yet.
 */
const shortcutPlatform = computed(() => readDocumentPlatform() ?? readSystemPlatform())

const railAppearanceValue = computed({
  get: () => appearance.railAppearance.value === 'translucent',
  set: (value: boolean) => appearance.setRailAppearance(value ? 'translucent' : 'solid'),
})

/**
 * Where terminal tabs open, section 4.7.
 *
 * The choice is a shell preference rather than a view one, but its control
 * belongs in settings with the rest of the shell's preferences, so the reader
 * finds it where they find the widths and the panel.
 */
const terminalPlacement = ref<TerminalPlacement>(readTerminalPlacement())
const terminalPlacementValue = computed({
  get: () => terminalPlacement.value,
  set: (value: string) => {
    if (value !== 'bottom' && value !== 'inspector') return
    terminalPlacement.value = value
    writeTerminalPlacement(value)
  },
})
const terminalPlacementOptions = computed<readonly AppSegmentOption[]>(() => [
  { id: 'bottom', label: t('settings.terminalPlacement.bottom') },
  { id: 'inspector', label: t('settings.terminalPlacement.inspector') },
])

const intensityOptions = computed(() => [
  { value: 'vivid', label: t('settings.intensity.vivid') },
  { value: 'calm', label: t('settings.intensity.calm') },
])

const intensityValue = computed({
  get: () => appearance.intensity.value,
  set: (value: string) => appearance.setIntensity(value as never),
})

/**
 * Reduced motion is a switch over three states: on, off, or whatever the OS
 * says. The switch is only ever the two explicit states, and the hint under it
 * reports which of the three is in force.
 */
const reducedMotionValue = computed({
  get: () => appearance.prefersReducedMotion.value,
  set: (value: boolean) => appearance.setReducedMotion(value ? 'true' : 'false'),
})

const reducedMotionSource = computed(() =>
  appearance.reducedMotion.value === null
    ? t('settings.reducedMotion.system')
    : t('settings.reducedMotion.explicit'),
)

/**
 * The preview shows the theme in force, so it prints that theme's accent for
 * the scheme that theme is set in - the light and dark halves of one scheme are
 * two colours, and the preview has to show the one the reader is looking at.
 */
const previewBefore = computed(() => [
  'const themePreview: ThemeConfig = {',
  '  surface: "sidebar",',
  '  accent: "#2563eb",',
  '  contrast: 42,',
  '};',
])

/**
 * Export, import and reset for the whole appearance profile.
 *
 * The file carries the values, not the storage keys, so it survives the local
 * storage layout changing underneath it. Import reports failure instead of
 * throwing: a file the user picked by hand is the most likely place for a
 * malformed profile to come from.
 */
const profileError = ref('')

function exportProfile(): void {
  profileError.value = ''
  const blob = new Blob([JSON.stringify(exportAppearanceProfile(), null, 2)], {
    type: 'application/json',
  })
  const url = URL.createObjectURL(blob)
  const link = document.createElement('a')
  link.href = url
  link.download = 'orchester-appearance.json'
  link.click()
  URL.revokeObjectURL(url)
}

function resetProfile(): void {
  profileError.value = ''
  resetAppearance()
}

async function importProfile(event: Event): Promise<void> {
  const input = event.target as HTMLInputElement
  const file = input.files?.[0]
  input.value = ''
  if (!file) return

  try {
    const parsed: unknown = JSON.parse(await file.text())
    profileError.value = importAppearanceProfile(parsed) ? '' : t('settings.table.invalid')
  } catch {
    profileError.value = t('settings.table.invalid')
  }
}

const previewAfter = computed(() => [
  'const themePreview: ThemeConfig = {',
  '  surface: "sidebar-elevated",',
  `  accent: "${accentFor(appearance.theme.value)}",`,
  '  contrast: 68,',
  '};',
])
</script>

<template>
  <div class="settings-view" data-testid="settings-view">
    <nav class="settings-view__nav" data-settings-nav :aria-label="t('settings.title')">
      <button
        class="settings-view__back"
        type="button"
        data-settings-back="workspace"
        @click="returnToWorkspace"
      >
        <ArrowLeft :size="15" aria-hidden="true" />
        {{ t('settings.backToApp') }}
      </button>

      <p class="settings-view__eyebrow">{{ t('settings.eyebrow') }}</p>
      <h1>{{ t('settings.title') }}</h1>

      <label class="settings-view__search" data-settings-search>
        <Search :size="15" aria-hidden="true" />
        <input
          v-model="settingsQuery"
          type="search"
          :placeholder="t('settings.search.placeholder')"
          :aria-label="t('settings.search.placeholder')"
        />
      </label>

      <p
        v-if="settingsQuery.trim().length > 0 && filteredSections.length === 0"
        class="settings-view__search-empty"
        data-settings-search-empty
        role="status"
      >
        {{ t('settings.search.empty') }}
      </p>

      <p v-if="generalSections.length > 0" class="settings-view__group">
        {{ t('settings.groups.personal') }}
      </p>
      <ul v-if="generalSections.length > 0" class="settings-view__list">
        <li v-for="section in generalSections" :key="section.id">
          <button
            class="settings-view__link"
            :class="{ 'settings-view__link--active': activeSection === section.id }"
            type="button"
            :data-settings-nav-link="section.id"
            :aria-current="activeSection === section.id"
            @click="activeSection = section.id"
          >
            <component :is="section.icon" :size="16" aria-hidden="true" />
            {{ t(section.labelKey) }}
          </button>
        </li>
      </ul>

      <p v-if="integrationSections.length > 0" class="settings-view__group">
        {{ t('settings.groups.integrations') }}
      </p>
      <ul v-if="integrationSections.length > 0" class="settings-view__list">
        <li v-for="section in integrationSections" :key="section.id">
          <button
            class="settings-view__link"
            :class="{ 'settings-view__link--active': activeSection === section.id }"
            type="button"
            :data-settings-nav-link="section.id"
            :aria-current="activeSection === section.id"
            @click="activeSection = section.id"
          >
            <component :is="section.icon" :size="16" aria-hidden="true" />
            {{ t(section.labelKey) }}
          </button>
        </li>
      </ul>
    </nav>

    <main class="settings-view__panels" :aria-label="t('settings.title')">
      <section
        class="settings-view__panel"
        data-settings-section="general"
        :aria-selected="activeSection === 'general'"
        :hidden="activeSection !== 'general'"
      >
        <h2>{{ t('settings.sections.general') }}</h2>
        <div class="settings-view__row">
          <div class="settings-view__row-copy">
            <strong>{{ t('settings.language.title') }}</strong>
            <span>{{ t('settings.language.description') }}</span>
          </div>
          <AppSelect
            v-model="localeValue"
            class="settings-view__control"
            :options="localeOptions"
            :aria-label="t('settings.language.title')"
          />
        </div>
        <div class="settings-view__row" data-settings-field="terminal-placement">
          <div class="settings-view__row-copy">
            <strong>{{ t('settings.terminalPlacement.title') }}</strong>
            <span>{{ t('settings.terminalPlacement.description') }}</span>
          </div>
          <AppSegmentedControl
            v-model="terminalPlacementValue"
            :options="terminalPlacementOptions"
            :ariaLabel="t('settings.terminalPlacement.title')"
          />
        </div>
      </section>

      <section
        class="settings-view__panel settings-view__panel--appearance"
        data-settings-section="appearance"
        :aria-selected="activeSection === 'appearance'"
        :hidden="activeSection !== 'appearance'"
      >
        <header class="settings-view__headline">
          <h2>{{ t('settings.sections.appearance') }}</h2>
          <p>{{ t('settings.appearance.description') }}</p>
        </header>

        <!-- The reference labels the cards rather than leaving three pictures
             to be guessed at, and the label is the group's accessible name too. -->
        <div class="settings-view__block" data-appearance-group="theme">
          <h3 class="settings-view__block-label">{{ t('settings.appearance.title') }}</h3>
          <div
            class="settings-view__cards"
            role="radiogroup"
            :aria-label="t('settings.appearance.title')"
          >
            <ThemePreviewCard
              v-for="card in themeCards"
              :key="card.value"
              :value="card.value"
              :label="card.label"
              :group-label="t('settings.appearance.title')"
              :selected="appearance.themePreference.value === card.value"
              @select="appearance.setThemePreference($event)"
            />
          </div>
        </div>

        <CodePreview :before="previewBefore" :after="previewAfter" />

        <!--
          One card per theme, as the reference draws them. Each reports the
          colours *it* resolves to rather than the ones in force, because the
          reader came here to set the theme they are not currently in as often
          as the one they are.
        -->
        <section
          v-for="mode in themeModes"
          :key="mode"
          class="settings-view__theme"
          :data-appearance-theme="mode"
        >
          <header class="settings-view__theme-head">
            <h3>{{ themeLabel(mode) }}</h3>
            <div class="settings-view__theme-actions">
              <AppButton
                variant="ghost"
                size="sm"
                data-action="import-theme"
                @click="importTrigger?.click()"
              >
                <Upload :size="14" aria-hidden="true" />
                {{ t('settings.table.import') }}
              </AppButton>
              <AppButton
                variant="ghost"
                size="sm"
                data-action="copy-theme"
                @click="copyTheme(mode)"
              >
                <Copy :size="14" aria-hidden="true" />
                {{ t('settings.appearance.copyTheme') }}
              </AppButton>
              <span
                class="settings-view__theme-swatch"
                data-theme-swatch
                :style="{ background: accentFor(mode), color: accentContrastFor(mode) }"
                aria-hidden="true"
              >
                {{ t('settings.appearance.sample') }}
              </span>
              <span class="settings-view__theme-choice" data-appearance-field="scheme">
                <AppSelect
                  :model-value="schemeFor(mode)"
                  :options="schemeOptions"
                  :aria-label="`${themeLabel(mode)}: ${t('settings.colorScheme.title')}`"
                  @update:model-value="appearance.setColorSchemeFor(mode, $event as ColorScheme)"
                />
              </span>
            </div>
          </header>

          <div class="settings-view__row" data-appearance-field="accent">
            <div class="settings-view__row-copy">
              <strong>{{ t('settings.colorScheme.title') }}</strong>
              <span>{{ t('settings.colorScheme.description') }}</span>
            </div>
            <div class="settings-view__row-control">
              <span
                class="settings-view__readout"
                data-color-readout
                :style="{ background: accentFor(mode) }"
                aria-hidden="true"
              />
              <code class="settings-view__hex" data-color-hex>{{ accentFor(mode) }}</code>
            </div>
          </div>

          <div class="settings-view__row" data-appearance-field="background">
            <div class="settings-view__row-copy">
              <strong>{{ t('settings.background.title') }}</strong>
              <span>{{ t('settings.background.description') }}</span>
            </div>
            <div class="settings-view__row-control">
              <span
                class="settings-view__readout"
                data-color-readout
                :style="{ background: coloursFor(mode).background }"
                aria-hidden="true"
              />
              <code class="settings-view__hex" data-color-hex>{{
                coloursFor(mode).background
              }}</code>
            </div>
          </div>

          <div class="settings-view__row" data-appearance-field="foreground">
            <div class="settings-view__row-copy">
              <strong>{{ t('settings.foreground.title') }}</strong>
              <span>{{ t('settings.foreground.description') }}</span>
            </div>
            <div class="settings-view__row-control">
              <span
                class="settings-view__readout"
                data-color-readout
                :style="{ background: coloursFor(mode).foreground }"
                aria-hidden="true"
              />
              <code class="settings-view__hex" data-color-hex>{{
                coloursFor(mode).foreground
              }}</code>
            </div>
          </div>
        </section>

        <div class="settings-view__table">
          <header class="settings-view__table-head">
            <h3>{{ t('settings.table.title') }}</h3>
            <div class="settings-view__table-actions" data-settings-actions>
              <AppButton
                variant="secondary"
                size="sm"
                data-action="import"
                @click="importTrigger?.click()"
              >
                <Upload :size="14" aria-hidden="true" />
                {{ t('settings.table.import') }}
              </AppButton>
              <AppButton variant="secondary" size="sm" data-action="export" @click="exportProfile">
                <Download :size="14" aria-hidden="true" />
                {{ t('settings.table.export') }}
              </AppButton>
              <AppButton variant="secondary" size="sm" data-action="reset" @click="resetProfile">
                <RotateCcw :size="14" aria-hidden="true" />
                {{ t('settings.table.reset') }}
              </AppButton>
            </div>
            <input
              ref="importTrigger"
              type="file"
              accept="application/json,.json"
              class="settings-view__file"
              :aria-label="t('settings.table.import')"
              @change="importProfile"
            />
          </header>
          <p v-if="profileError" class="settings-view__note" role="alert">{{ profileError }}</p>

          <div class="settings-view__row" data-appearance-field="intensity">
            <div class="settings-view__row-copy">
              <strong>{{ t('settings.intensity.title') }}</strong>
              <span>{{ t('settings.intensity.description') }}</span>
            </div>
            <AppSelect
              v-model="intensityValue"
              class="settings-view__control"
              :options="intensityOptions"
              :aria-label="t('settings.intensity.title')"
            />
          </div>

          <div class="settings-view__row" data-appearance-field="reduced-motion">
            <div class="settings-view__row-copy">
              <strong>{{ t('settings.reducedMotion.title') }}</strong>
              <span>{{ t('settings.reducedMotion.description') }}</span>
            </div>
            <div class="settings-view__row-control">
              <AppBadge tone="neutral" mono>{{ reducedMotionSource }}</AppBadge>
              <AppSwitch
                v-model="reducedMotionValue"
                :label="t('settings.reducedMotion.title')"
              />
            </div>
          </div>

          <div class="settings-view__row" data-appearance-field="ui-font">
            <div class="settings-view__row-copy">
              <strong>{{ t('settings.uiFont.title') }}</strong>
              <span>{{ t('settings.uiFont.description') }}</span>
            </div>
            <div class="settings-view__row-control">
              <AppSelect
                v-model="uiFontValue"
                class="settings-view__control"
                :options="uiFontOptions"
                :aria-label="t('settings.uiFont.title')"
              />
              <AppSelect
                v-model="uiFontWeightValue"
                class="settings-view__control settings-view__control--narrow"
                :options="fontWeightOptions"
                :aria-label="t('settings.fontWeight.title')"
              />
            </div>
          </div>

          <div class="settings-view__row" data-appearance-field="content-font">
            <div class="settings-view__row-copy">
              <strong>{{ t('settings.contentFont.title') }}</strong>
              <span>{{ t('settings.contentFont.description') }}</span>
            </div>
            <div class="settings-view__row-control">
              <AppSelect
                v-model="contentFontValue"
                class="settings-view__control"
                :options="contentFontOptions"
                :aria-label="t('settings.contentFont.title')"
              />
              <AppSelect
                v-model="contentFontWeightValue"
                class="settings-view__control settings-view__control--narrow"
                :options="fontWeightOptions"
                :aria-label="t('settings.fontWeight.title')"
              />
            </div>
          </div>

          <div class="settings-view__row" data-appearance-field="rail-appearance">
            <div class="settings-view__row-copy">
              <strong>{{ t('settings.translucentRail.title') }}</strong>
              <span>{{ t('settings.translucentRail.description') }}</span>
            </div>
            <div class="settings-view__row-control">
              <AppBadge v-if="!supportsTranslucency" tone="neutral" mono>
                {{ appearance.surface.value }}
              </AppBadge>
              <AppSwitch
                v-model="railAppearanceValue"
                :disabled="!supportsTranslucency"
                :label="t('settings.translucentRail.title')"
              />
            </div>
          </div>

          <div class="settings-view__row" data-appearance-field="surface">
            <div class="settings-view__row-copy">
              <strong>{{ t('settings.surface.title') }}</strong>
              <span>{{ t('settings.surface.description') }}</span>
            </div>
            <AppBadge tone="neutral" mono>{{ appearance.surface.value }}</AppBadge>
          </div>
        </div>
      </section>

      <section
        class="settings-view__panel"
        data-settings-section="notifications"
        :aria-selected="activeSection === 'notifications'"
        :hidden="activeSection !== 'notifications'"
      >
        <h2>{{ t('settings.sections.notifications') }}</h2>
        <EmptyState
          :title="t('settings.notifications.title')"
          :description="t('settings.notifications.description')"
        />
      </section>

      <section
        class="settings-view__panel"
        data-settings-section="import"
        :aria-selected="activeSection === 'import'"
        :hidden="activeSection !== 'import'"
      >
        <h2>{{ t('settings.sections.import') }}</h2>
        <EmptyState
          :title="t('settings.import.title')"
          :description="t('settings.import.description')"
        />
      </section>

      <section
        class="settings-view__panel"
        data-settings-section="profile"
        :aria-selected="activeSection === 'profile'"
        :hidden="activeSection !== 'profile'"
      >
        <h2>{{ t('settings.sections.profile') }}</h2>
        <EmptyState
          :title="t('settings.profile.title')"
          :description="t('settings.profile.description')"
        />
      </section>

      <section
        class="settings-view__panel"
        data-settings-section="keybindings"
        :aria-selected="activeSection === 'keybindings'"
        :hidden="activeSection !== 'keybindings'"
      >
        <h2>{{ t('settings.sections.keybindings') }}</h2>
        <p class="settings-view__note">{{ t('settings.keybindings.description') }}</p>
        <ShortcutEditor
          :registry="shortcutRegistry"
          :platform="shortcutPlatform"
          :search-label="t('settings.keybindings.search')"
          :search-placeholder="t('settings.keybindings.search')"
          :record-label="t('settings.keybindings.change')"
          :capture-label="t('settings.keybindings.capture')"
          :reset-label="t('settings.keybindings.reset')"
          :empty-label="t('settings.keybindings.empty')"
        />
      </section>

      <section
        class="settings-view__panel"
        data-settings-section="pet"
        :aria-selected="activeSection === 'pet'"
        :hidden="activeSection !== 'pet'"
      >
        <h2>{{ t('pet.title') }}</h2>
        <div class="settings-view__row">
          <div class="settings-view__row-copy">
            <strong>{{ t('pet.visibility.title') }}</strong>
            <span>{{ t('pet.visibility.description') }}</span>
          </div>
          <AppSwitch v-model="petVisible" :label="t('pet.visibility.title')" />
        </div>
        <p class="settings-view__note">{{ t('pet.description') }}</p>
      </section>

      <section
        class="settings-view__panel"
        data-settings-section="providers"
        :aria-selected="activeSection === 'providers'"
        :hidden="activeSection !== 'providers'"
      >
        <h2>{{ t('settings.sections.providers') }}</h2>
        <p class="settings-view__note">{{ t('settings.providers.description') }}</p>
      </section>

      <section
        class="settings-view__panel"
        data-settings-section="about"
        :aria-selected="activeSection === 'about'"
        :hidden="activeSection !== 'about'"
      >
        <h2>{{ t('settings.sections.about') }}</h2>
        <p class="settings-view__note">{{ t('settings.about.description') }}</p>
      </section>
    </main>
  </div>
</template>

<style scoped>
.settings-view {
  display: grid;
  grid-template-columns: minmax(14rem, 17rem) minmax(0, 1fr);
  min-block-size: calc(100vh - var(--app-top-chrome-height, var(--header-height)));
  background: var(--color-surface-tertiary);
}

.settings-view__nav {
  padding: var(--space-4) var(--space-3);
  border-inline-end: 1px solid var(--color-border-default);
  background: var(--color-surface-secondary);
}

.settings-view__eyebrow {
  margin: 0 0 var(--space-1);
  padding-inline: var(--space-2);
  color: var(--color-text-tertiary);
  font-family: var(--font-mono);
  font-size: var(--text-xs);
  text-transform: uppercase;
}

.settings-view__nav h1 {
  margin: 0 0 var(--space-4);
  padding-inline: var(--space-2);
  font-size: var(--text-lg);
}

/* The way back sits above the nav it belongs to, the way the reference heads
   its settings column with it rather than leaving the reader to the browser. */
.settings-view__back {
  display: flex;
  inline-size: 100%;
  min-block-size: max(var(--density-row-height), var(--hit-target-min, 32px));
  align-items: center;
  gap: var(--space-2);
  margin: 0 0 var(--space-4);
  padding-inline: var(--space-2);
  border: 0;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--color-text-secondary);
  font: inherit;
  font-size: var(--text-sm);
  text-align: start;
  cursor: pointer;
}

.settings-view__back:hover {
  background: var(--color-surface-base);
  color: var(--color-text-primary);
}

.settings-view__back:focus-visible {
  outline: 2px solid var(--color-border-focus);
  outline-offset: 2px;
}

.settings-view__group {
  margin: var(--space-4) var(--space-2) var(--space-2);
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
  font-weight: var(--weight-medium);
}

.settings-view__search {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0 var(--space-2) var(--space-2);
  padding: var(--space-1) var(--space-2);
  border: 1px solid var(--color-border-control);
  border-radius: var(--radius-sm);
  background: var(--color-surface-base);
  color: var(--color-text-tertiary);
}

.settings-view__search:focus-within {
  border-color: var(--color-border-focus);
  /* The wrapper painting its own boundary is a replacement for the ring on
     the input, which is why the input can drop the shared outline. The
     replacement has to be at least as visible: the focus colour on the ring
     and on this border are the same token. */
  box-shadow: 0 0 0 1px var(--color-border-focus);
}

.settings-view__search input {
  inline-size: 100%;
  min-inline-size: 0;
  border: 0;
  background: transparent;
  color: var(--color-text-primary);
  font: inherit;
  font-size: var(--text-sm);
}

.settings-view__search input:focus {
  /* The field is borderless and the wrapper draws the focus boundary, so the
     default ring would double it. The wrapper's `:focus-within` rule above is
     the replacement §7 requires. */
  outline: none;
}

.settings-view__search-empty {
  margin: 0 var(--space-2) var(--space-2);
  color: var(--color-text-tertiary);
  font-size: var(--text-sm);
}

.settings-view__list {
  display: grid;
  gap: 2px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.settings-view__link {
  display: flex;
  inline-size: 100%;
  /* The density row height is the visual; the hit-target floor is the promise.
     Compact drops the row to 26 px, so the floor has to outrank it rather than
     ride along beside it. */
  min-block-size: max(var(--density-row-height), var(--hit-target-min, 32px));
  align-items: center;
  gap: var(--space-3);
  padding-inline: var(--space-2);
  border: 0;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--color-text-secondary);
  font: inherit;
  font-size: var(--text-sm);
  text-align: start;
  cursor: pointer;
}

.settings-view__link:hover {
  background: var(--color-surface-base);
  color: var(--color-text-primary);
}

.settings-view__link--active {
  background: var(--color-accent-muted);
  color: var(--color-text-primary);
}

.settings-view__panels {
  display: grid;
  align-content: start;
  gap: var(--space-4);
  min-inline-size: 0;
  padding: var(--space-6) var(--space-8);
  overflow: auto;
}

.settings-view__panel {
  display: grid;
  gap: var(--space-4);
  inline-size: min(100%, 68rem);
  margin-inline: auto;
}

.settings-view__panel[hidden] {
  display: none;
}

.settings-view__headline h2,
.settings-view__panel > h2 {
  margin: 0;
  font-size: var(--text-xl);
  letter-spacing: -0.01em;
}

.settings-view__headline p {
  margin: var(--space-1) 0 0;
  color: var(--color-text-secondary);
  font-size: var(--text-sm);
}

/* The reference labels the cards above them rather than leaving three
   pictures to be guessed at. */
.settings-view__block {
  display: grid;
  gap: var(--space-3);
}

.settings-view__block-label {
  margin: 0;
  color: var(--color-text-secondary);
  font-size: var(--text-sm);
  font-weight: var(--weight-medium);
}

.settings-view__cards {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--space-4);
}

/* One theme's card: the head carries the theme's own actions and the choice,
   the rows below report what that theme resolves to. */
.settings-view__theme {
  overflow: hidden;
  border: 1px solid var(--color-border-default);
  border-radius: var(--radius-md);
  background: var(--color-surface-base);
}

.settings-view__theme-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  padding: var(--space-3) var(--space-4);
  border-block-end: 1px solid var(--color-border-default);
}

.settings-view__theme-head h3 {
  margin: 0;
  font-size: var(--text-sm);
  font-weight: var(--weight-medium);
}

.settings-view__theme-actions {
  display: flex;
  flex: 0 0 auto;
  align-items: center;
  gap: var(--space-2);
}

.settings-view__theme-actions :deep(.app-button__label) {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  white-space: nowrap;
}

/* The reference's swatch spells "Aa" in the theme's own accent, so the card
   shows the face as well as the colour. */
.settings-view__theme-swatch {
  display: inline-grid;
  inline-size: 1.75rem;
  block-size: 1.75rem;
  flex: 0 0 auto;
  place-items: center;
  border: 1px solid var(--color-border-emphasis);
  border-radius: var(--radius-sm);
  color: var(--color-accent-contrast);
  font-size: var(--text-xs);
  font-weight: var(--weight-medium);
}

.settings-view__theme-choice {
  display: inline-flex;
  inline-size: 11rem;
}

.settings-view__theme .settings-view__row + .settings-view__row {
  border-block-start: 1px solid var(--color-border-default);
}

.settings-view__table {
  overflow: hidden;
  border: 1px solid var(--color-border-default);
  border-radius: var(--radius-md);
  background: var(--color-surface-base);
}

.settings-view__table-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  padding: var(--space-3) var(--space-4);
  border-block-end: 1px solid var(--color-border-default);
}

.settings-view__table-actions {
  display: flex;
  flex: 0 0 auto;
  align-items: center;
  gap: var(--space-2);
}

/* The reset makes every svg a block, so an icon beside a label breaks onto its
   own line. The label is turned into a row here rather than changing the reset,
   which the rest of the product relies on. */
.settings-view__table-actions :deep(.app-button__label) {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  white-space: nowrap;
}

/* The picker is opened from the button beside it; showing a bare file control
   in the header would be a second, uglier way to do the same thing. */
.settings-view__file {
  position: absolute;
  inline-size: 1px;
  block-size: 1px;
  overflow: hidden;
  clip-path: inset(50%);
}

.settings-view__table-head h3 {
  margin: 0;
  font-size: var(--text-sm);
  font-weight: var(--weight-medium);
}

.settings-view__row {
  display: flex;
  min-block-size: 3rem;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
  padding: var(--space-2) var(--space-4);
}

.settings-view__table .settings-view__row + .settings-view__row {
  border-block-start: 1px solid var(--color-border-default);
}

.settings-view__row-copy {
  display: grid;
  gap: 2px;
  min-inline-size: 0;
}

.settings-view__row-copy strong {
  font-size: var(--text-sm);
  font-weight: var(--weight-medium);
}

.settings-view__row-copy span,
.settings-view__note {
  margin: 0;
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
}

.settings-view__row-control {
  display: flex;
  flex: 0 0 auto;
  align-items: center;
  gap: var(--space-2);
}

.settings-view__control {
  inline-size: 13rem;
}

/* The weight select is one word wide in every language, so it gets a third of
   the row rather than competing with the face beside it. */
.settings-view__control--narrow {
  inline-size: 7rem;
}

/* The background and foreground rows report what the theme already resolves
   to. They deliberately hold no control: the two colours are an outcome of the
   theme choice, and a hex picker here would be a second theme editor. */
.settings-view__readout {
  display: inline-block;
  inline-size: 1.4rem;
  block-size: 1.4rem;
  flex: 0 0 auto;
  border: 1px solid var(--color-border-emphasis);
  border-radius: var(--radius-full);
}

/* The reference prints the value beside the swatch, so the row reports what
   the theme resolved to rather than only hinting at it with a dot. */
.settings-view__hex {
  color: var(--color-text-secondary);
  font-family: var(--font-mono);
  font-size: var(--text-xs);
}

@media (max-width: 900px) {
  .settings-view {
    grid-template-columns: minmax(0, 1fr);
  }

  .settings-view__nav {
    border-inline-end: 0;
    border-block-end: 1px solid var(--color-border-default);
  }

  .settings-view__cards {
    grid-template-columns: minmax(0, 1fr);
  }

  .settings-view__panels {
    padding: var(--space-4);
  }

  .settings-view__row {
    align-items: stretch;
    flex-direction: column;
  }

  .settings-view__control {
    inline-size: 100%;
  }
}
</style>
