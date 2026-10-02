<script setup lang="ts">
import { PanelBottom, PanelLeft, PawPrint, Settings2 } from '@lucide/vue'

import mark from '../../assets/orchester-mark.png'

defineProps<{
  label: string
  newSessionLabel: string
  sessionsLabel: string
  settingsLabel: string
  companionLabel: string
  bottomPanelLabel: string
  bottomPanelExpanded: boolean
  sessionsExpanded: boolean
  homeActive: boolean
  companionVisible: boolean
}>()

defineEmits<{
  newSession: []
  toggleSessions: []
  openSettings: []
  toggleCompanion: []
  toggleBottomPanel: []
}>()
</script>

<template>
  <nav class="workspace-navigation" data-workspace-navigation :aria-label="label">
    <button class="workspace-navigation__button workspace-navigation__home" type="button"
      :aria-label="newSessionLabel" :title="newSessionLabel" :aria-pressed="homeActive"
      @click="$emit('newSession')">
      <img :src="mark" alt="" draggable="false" />
    </button>
    <button class="workspace-navigation__button" type="button" :aria-label="sessionsLabel"
      :title="sessionsLabel" :aria-expanded="sessionsExpanded" @click="$emit('toggleSessions')">
      <PanelLeft :size="19" aria-hidden="true" />
    </button>
    <div class="workspace-navigation__footer">
      <button class="workspace-navigation__button" type="button" :aria-label="bottomPanelLabel"
        :title="bottomPanelLabel" :aria-expanded="bottomPanelExpanded" @click="$emit('toggleBottomPanel')">
        <PanelBottom :size="19" aria-hidden="true" />
      </button>
      <button class="workspace-navigation__button" type="button" :aria-label="companionLabel"
        :title="companionLabel" :aria-pressed="companionVisible" @click="$emit('toggleCompanion')">
        <PawPrint :size="19" aria-hidden="true" />
      </button>
      <button class="workspace-navigation__button" type="button" :aria-label="settingsLabel"
        :title="settingsLabel" @click="$emit('openSettings')">
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
.workspace-navigation__home[aria-pressed='true'] {
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
