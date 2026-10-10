<script setup lang="ts">
import { House, Clock3, Puzzle, PawPrint, Settings2, Sun, Moon } from '@lucide/vue'


defineProps<{
  label: string
  homeLabel: string
  schedulesLabel: string
  pluginsLabel: string
  themeLabel: string
  settingsLabel: string
  companionLabel: string
  activePage: string
  dark: boolean
  companionVisible: boolean
}>()

defineEmits<{
  home: []
  openSchedules: []
  openPlugins: []
  toggleTheme: []
  openSettings: []
  toggleCompanion: []
}>()
</script>

<template>
  <nav class="workspace-navigation" data-workspace-navigation :aria-label="label">
    <button class="workspace-navigation__button workspace-navigation__home" type="button"
      :aria-label="homeLabel" :title="homeLabel" :aria-current="activePage === 'workspace' ? 'page' : false"
      @click="$emit('home')">
      <House :size="20" aria-hidden="true" />
    </button>
    <button class="workspace-navigation__button" type="button" :aria-label="schedulesLabel"
      :title="schedulesLabel" :aria-current="activePage === 'schedules' ? 'page' : false" @click="$emit('openSchedules')">
      <Clock3 :size="20" aria-hidden="true" />
    </button>
    <button class="workspace-navigation__button" type="button" :aria-label="pluginsLabel"
      :title="pluginsLabel" :aria-current="activePage === 'plugins' ? 'page' : false" @click="$emit('openPlugins')">
      <Puzzle :size="20" aria-hidden="true" />
    </button>
    <div class="workspace-navigation__footer">
      <button class="workspace-navigation__button" type="button" :aria-label="companionLabel"
        :title="companionLabel" :aria-pressed="companionVisible" @click="$emit('toggleCompanion')">
        <PawPrint :size="19" aria-hidden="true" />
      </button>
      <button class="workspace-navigation__button" type="button" :aria-label="themeLabel"
        :title="themeLabel" :aria-pressed="dark" @click="$emit('toggleTheme')">
        <Moon v-if="dark" :size="19" aria-hidden="true" />
        <Sun v-else :size="19" aria-hidden="true" />
      </button>
      <button class="workspace-navigation__button" type="button" :aria-label="settingsLabel"
        :title="settingsLabel" :aria-current="activePage === 'settings' ? 'page' : false" @click="$emit('openSettings')">
        <Settings2 :size="19" aria-hidden="true" />
      </button>
    </div>
  </nav>
</template>

<style scoped>
.workspace-navigation {
  display: flex;
  block-size: 100%;
  flex-direction: column;
  align-items: center;
  gap: var(--space-3);
  padding-block: var(--space-3);
}

.workspace-navigation__button {
  display: grid;
  inline-size: 36px;
  block-size: 36px;
  flex: 0 0 36px;
  place-items: center;
  padding: 0;
  border: 0;
  border-radius: 11px;
  background: transparent;
  color: var(--color-text-secondary);
  cursor: pointer;
  transition: background var(--transition-fast) var(--ease-out), color var(--transition-fast) var(--ease-out);
}

.workspace-navigation__button:hover,
.workspace-navigation__button[aria-current='page'] {
  background: var(--color-bg-element);
  color: var(--color-text-primary);
}

.workspace-navigation__button:focus-visible {
  outline: 2px solid var(--color-border-focus);
  outline-offset: 2px;
}

.workspace-navigation__home img {
  inline-size: 26px;
  block-size: 26px;
  border-radius: 8px;
  object-fit: cover;
}

.workspace-navigation__footer {
  display: grid;
  gap: var(--space-2);
  margin-block-start: auto;
}
</style>
