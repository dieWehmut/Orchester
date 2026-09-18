<script setup lang="ts">
import { Info, Palette, PawPrint, Plug, Settings2 } from '@lucide/vue'
import {
  AppBadge,
  AppSegmentedControl,
  AppSelect,
  AppSwitch,
  ColorSchemePicker,
  ThemeToggle,
  initAppearance,
  useAppearance,
  type AppearanceApi,
  type ThemeMode,
} from '@orchester/design'
import { computed, ref } from 'vue'

import { usePetVisibility } from '../features/pet'
import { useI18n } from '../i18n'

type SettingsSection = 'general' | 'appearance' | 'pet' | 'providers' | 'about'

const { t, locale, setLocale } = useI18n()

initAppearance()
const appearance: AppearanceApi = useAppearance()
const petVisibility = usePetVisibility()

const petVisible = computed({
  get: () => petVisibility.visible.value,
  set: (value: boolean) => (value ? petVisibility.show() : petVisibility.hide()),
})

const activeSection = ref<SettingsSection>('general')

const sections: { id: SettingsSection; labelKey: Parameters<typeof t>[0]; icon: typeof Info }[] = [
  { id: 'general', labelKey: 'settings.sections.general', icon: Settings2 },
  { id: 'appearance', labelKey: 'settings.sections.appearance', icon: Palette },
  { id: 'pet', labelKey: 'pet.title', icon: PawPrint },
  { id: 'providers', labelKey: 'settings.sections.providers', icon: Plug },
  { id: 'about', labelKey: 'settings.sections.about', icon: Info },
]

const localeOptions = computed(() => [
  { value: 'en', label: t('settings.locale.en') },
  { value: 'zh-CN', label: t('settings.locale.zhCN') },
  { value: 'zh-TW', label: t('settings.locale.zhTW') },
])

const localeValue = computed({
  get: () => locale.value,
  set: (value: string) => setLocale(value),
})

const appearanceSummary = computed(() =>
  appearance.isDark.value ? t('settings.appearance.dark') : t('settings.appearance.light'),
)

const themeValue = computed({
  get: () => appearance.theme.value,
  set: (value: string) => appearance.setTheme(value as ThemeMode),
})
</script>

<template>
  <div class="settings-view" data-testid="settings-view">
    <nav class="settings-view__nav" data-settings-nav :aria-label="t('settings.title')">
      <p class="settings-view__eyebrow">{{ t('settings.eyebrow') }}</p>
      <h1>{{ t('settings.title') }}</h1>
      <ul class="settings-view__list">
        <li v-for="section in sections" :key="section.id">
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

    <div class="settings-view__panels">
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
            :options="localeOptions"
            :aria-label="t('settings.language.title')"
          />
        </div>
        <div class="settings-view__row">
          <div class="settings-view__row-copy">
            <strong>{{ t('settings.theme.title') }}</strong>
            <span>{{ t('settings.theme.description') }}</span>
          </div>
          <div class="settings-view__row-control">
            <AppBadge tone="neutral" mono>{{ appearanceSummary }}</AppBadge>
            <ThemeToggle
              :label-dark="t('settings.theme.toLight')"
              :label-light="t('settings.theme.toDark')"
            />
          </div>
        </div>
      </section>

      <section
        class="settings-view__panel"
        data-settings-section="appearance"
        :aria-selected="activeSection === 'appearance'"
        :hidden="activeSection !== 'appearance'"
      >
        <h2>{{ t('settings.sections.appearance') }}</h2>
        <div class="settings-view__row settings-view__row--stacked">
          <div class="settings-view__row-copy">
            <strong>{{ t('settings.colorScheme.title') }}</strong>
            <span>{{ t('settings.colorScheme.description') }}</span>
          </div>
          <ColorSchemePicker />
        </div>
        <div class="settings-view__row">
          <div class="settings-view__row-copy">
            <strong>{{ t('settings.theme.title') }}</strong>
            <span>{{ t('settings.theme.description') }}</span>
          </div>
          <AppSegmentedControl
            v-model="themeValue"
            :options="[
              { id: 'dark', label: t('settings.appearance.dark') },
              { id: 'light', label: t('settings.appearance.light') },
            ]"
            :ariaLabel="t('settings.theme.title')"
          />
        </div>
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
    </div>
  </div>
</template>

<style scoped>
.settings-view {
  display: grid;
  grid-template-columns: minmax(15rem, 18rem) minmax(0, 1fr);
  min-block-size: calc(100vh - var(--app-top-chrome-height, var(--header-height)));
  background: var(--color-bg-base);
}

.settings-view__nav {
  padding: var(--space-6) var(--space-4);
  border-inline-end: 1px solid var(--color-border-base);
  background: var(--color-bg-surface);
}

.settings-view__eyebrow {
  margin: 0 0 var(--space-1);
  color: var(--color-text-tertiary);
  font-family: var(--font-mono);
  font-size: var(--text-xs);
  text-transform: uppercase;
}

.settings-view__nav h1 {
  margin: 0 0 var(--space-4);
  font-size: var(--text-lg);
}

.settings-view__list {
  display: grid;
  gap: var(--space-1);
  margin: 0;
  padding: 0;
  list-style: none;
}

.settings-view__link {
  display: flex;
  inline-size: 100%;
  min-block-size: var(--control-height-md);
  align-items: center;
  gap: var(--space-2);
  padding-inline: var(--space-3);
  border: 1px solid transparent;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--color-text-secondary);
  font: inherit;
  font-size: var(--text-sm);
  text-align: start;
  cursor: pointer;
}

.settings-view__link:hover {
  background: var(--color-bg-element);
  color: var(--color-text-primary);
}

.settings-view__link--active {
  border-color: var(--color-accent-border);
  background: var(--color-accent-muted);
  color: var(--color-text-primary);
}

.settings-view__panels {
  display: grid;
  align-content: start;
  gap: var(--space-4);
  padding: var(--space-6);
}

.settings-view__panel {
  display: grid;
  gap: var(--space-3);
  max-inline-size: 46rem;
  padding: var(--space-5);
  border: 1px solid var(--color-border-base);
  border-radius: var(--radius-md);
  background: var(--color-bg-surface);
}

.settings-view__panel h2 {
  margin: 0;
  font-size: var(--text-base);
}

.settings-view__row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
  padding-block: var(--space-3);
  border-block-start: 1px solid var(--color-border-base);
}

.settings-view__row--stacked {
  flex-direction: column;
  align-items: stretch;
}

.settings-view__row-copy {
  display: grid;
  gap: var(--space-1);
}

.settings-view__row-copy strong {
  font-size: var(--text-sm);
}

.settings-view__row-copy span,
.settings-view__note {
  margin: 0;
  color: var(--color-text-secondary);
  font-size: var(--text-sm);
}

.settings-view__row-control {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

@media (max-width: 900px) {
  .settings-view {
    grid-template-columns: minmax(0, 1fr);
  }

  .settings-view__nav {
    border-inline-end: 0;
    border-block-end: 1px solid var(--color-border-base);
  }
}
</style>
