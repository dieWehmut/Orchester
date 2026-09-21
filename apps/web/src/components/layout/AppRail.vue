<script setup lang="ts">
import { ChevronDown, ChevronRight, CircleUser, Settings, SquarePen } from '@lucide/vue'
import { computed, ref } from 'vue'

import { AppButton, AppMenu, IconButton, type AppMenuItem } from '@orchester/design'

import mark from '../../assets/orchester-mark.png'

/**
 * The rail answers the reference sidebar rather than a run of loose headings.
 *
 * Three things in the reference are structural: the product name is a row with
 * its own disclosure, each list is headed by a control that folds it, and the
 * account at the foot opens a menu instead of sitting beside a lone gear. The
 * folding is local state because it is a reading preference for one session,
 * not a setting the runtime should store.
 */
const collapsed = ref<Record<string, boolean>>({ pinned: false, projects: false, agents: false })

function toggleSection(id: string): void {
  collapsed.value = { ...collapsed.value, [id]: !collapsed.value[id] }
}

const props = withDefaults(
  defineProps<{
    productName: string
    workspaceName?: string | null
    newSessionLabel: string
    projectsLabel: string
    sessionsLabel: string
    fleetLabel: string
    accountName?: string | null
    accountHint?: string | null
    settingsLabel?: string
    companionLabel?: string
  }>(),
  {
    workspaceName: null,
    accountName: null,
    accountHint: null,
    settingsLabel: 'Settings',
    companionLabel: '',
  },
)

const emit = defineEmits<{
  newSession: []
  openSettings: []
  toggleCompanion: []
}>()

/**
 * The account menu, as the reference draws it.
 *
 * The reference's menu hangs off the account row and mixes the shell's own
 * destinations with the runtime it is attached to. This registry is local and
 * has no sign-out to offer, so the menu carries the two things it can honestly
 * answer: the companion's visibility, and the settings route. The labels are
 * props rather than words, because the surface that renders them is the one
 * that can translate them.
 */
const accountItems = computed<AppMenuItem[]>(() =>
  [
    props.companionLabel ? { id: 'companion', label: props.companionLabel } : null,
    { id: 'settings', label: props.settingsLabel },
  ].filter((item): item is AppMenuItem => item !== null),
)

function chooseAccountItem(id: string): void {
  if (id === 'settings') emit('openSettings')
  else if (id === 'companion') emit('toggleCompanion')
}
</script>

<template>
  <div class="app-rail" data-app-rail>
    <div class="app-rail__section app-rail__product" data-rail-section="brand" data-rail-product>
      <span class="app-rail__mark-slot" aria-hidden="true">
        <img class="app-rail__mark" data-rail-mark :src="mark" alt="" draggable="false" />
      </span>
      <span class="app-rail__identity">
        <strong>{{ productName }}</strong>
        <span v-if="workspaceName">{{ workspaceName }}</span>
      </span>
      <ChevronDown
        class="app-rail__product-disclosure"
        :size="16"
        aria-hidden="true"
        data-rail-product-disclosure
      />
    </div>

    <div class="app-rail__section" data-rail-section="primary">
      <AppButton
        block
        variant="secondary"
        data-rail-action="new-session"
        @click="$emit('newSession')"
      >
        <SquarePen :size="16" aria-hidden="true" />
        {{ newSessionLabel }}
      </AppButton>
    </div>

    <div
      class="app-rail__section app-rail__section--scroll"
      data-rail-section="projects"
      data-rail-heading="pinned"
    >
      <button
        class="app-rail__heading"
        type="button"
        data-rail-disclosure="pinned"
        :aria-expanded="!collapsed.pinned"
        @click="toggleSection('pinned')"
      >
        <ChevronDown v-if="!collapsed.pinned" :size="14" aria-hidden="true" />
        <ChevronRight v-else :size="14" aria-hidden="true" />
        {{ projectsLabel }}
      </button>
      <div v-if="!collapsed.pinned" class="app-rail__entries">
        <slot name="projects" />
      </div>
    </div>

    <div
      class="app-rail__section app-rail__section--scroll"
      data-rail-section="sessions"
      data-rail-heading="projects"
    >
      <button
        class="app-rail__heading"
        type="button"
        data-rail-disclosure="projects"
        :aria-expanded="!collapsed.projects"
        @click="toggleSection('projects')"
      >
        <ChevronDown v-if="!collapsed.projects" :size="14" aria-hidden="true" />
        <ChevronRight v-else :size="14" aria-hidden="true" />
        {{ sessionsLabel }}
      </button>
      <div v-if="!collapsed.projects" class="app-rail__entries">
        <slot name="sessions" />
      </div>
    </div>

    <div
      class="app-rail__section app-rail__section--footer"
      data-rail-section="fleet"
      data-rail-heading="agents"
    >
      <button
        class="app-rail__heading"
        type="button"
        data-rail-disclosure="agents"
        :aria-expanded="!collapsed.agents"
        @click="toggleSection('agents')"
      >
        <ChevronDown v-if="!collapsed.agents" :size="14" aria-hidden="true" />
        <ChevronRight v-else :size="14" aria-hidden="true" />
        {{ fleetLabel }}
      </button>
      <div v-if="!collapsed.agents" class="app-rail__entries">
        <slot name="fleet" />
      </div>
    </div>

    <div class="app-rail__section app-rail__account" data-rail-section="account">
      <AppMenu
        class="app-rail__account-menu"
        data-rail-account-menu
        placement="top"
        :label="accountName || productName"
        :items="accountItems"
        @select="chooseAccountItem"
      >
        <template #trigger>
          <span class="app-rail__account-identity" data-rail-account>
            <CircleUser :size="18" aria-hidden="true" />
            <span class="app-rail__account-copy">
              <strong>{{ accountName || productName }}</strong>
              <span v-if="accountHint">{{ accountHint }}</span>
            </span>
          </span>
        </template>
      </AppMenu>
      <IconButton
        :label="settingsLabel"
        data-rail-action="settings"
        @click="$emit('openSettings')"
      >
        <Settings :size="16" aria-hidden="true" />
      </IconButton>
    </div>
  </div>
</template>

<style scoped>
.app-rail {
  display: grid;
  grid-template-rows: auto auto minmax(0, 1fr) minmax(0, 1fr) auto auto;
  block-size: 100%;
  min-block-size: 0;
  background: var(--rail-surface);
  backdrop-filter: var(--rail-blur);
}

.app-rail__section {
  min-inline-size: 0;
  padding: var(--space-3);
}

.app-rail__section--scroll {
  min-block-size: 0;
  overflow: auto;
}

.app-rail__section--footer {
  border-block-start: 1px solid var(--color-border-base);
  background: color-mix(in srgb, var(--color-bg-base) 6%, var(--rail-surface));
}

.app-rail__account {
  display: flex;
  min-inline-size: 0;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
  border-block-start: 1px solid var(--color-border-base);
  background: color-mix(in srgb, var(--color-bg-base) 10%, var(--rail-surface));
}

.app-rail__account-identity {
  display: flex;
  min-inline-size: 0;
  align-items: center;
  gap: var(--space-3);
  color: var(--color-text-secondary);
}

.app-rail__account-copy {
  display: grid;
  min-inline-size: 0;
  line-height: var(--leading-tight);
}

.app-rail__account-copy strong {
  overflow: hidden;
  color: var(--color-text-primary);
  font-size: var(--text-sm);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.app-rail__account-copy span {
  overflow: hidden;
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.app-rail__section[data-rail-section='brand'] {
  display: flex;
  min-inline-size: 0;
  align-items: center;
  gap: var(--space-3);
  border-block-end: 1px solid var(--color-border-base);
}

.app-rail__mark-slot {
  display: grid;
  inline-size: 32px;
  block-size: 32px;
  flex: 0 0 32px;
  place-items: center;
  overflow: hidden;
  border: 1px solid var(--color-border-base);
  border-radius: var(--radius-sm);
}

.app-rail__mark {
  inline-size: 100%;
  block-size: 100%;
  object-fit: cover;
  user-select: none;
}

.app-rail__identity {
  display: grid;
  min-inline-size: 0;
  line-height: var(--leading-tight);
}

.app-rail__identity strong {
  font-size: var(--text-sm);
}

.app-rail__identity span {
  overflow: hidden;
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* The heading is the folding control, so it is a button that happens to be
   set in the heading's type rather than a heading beside a separate chevron.
   A list whose heading is not also the control asks the reader to find two
   things to fold one. */
.app-rail__heading {
  display: flex;
  inline-size: 100%;
  min-block-size: var(--hit-target-min, 32px);
  align-items: center;
  gap: var(--space-1);
  margin: 0 0 var(--space-1);
  padding: 0 var(--space-1);
  border: 0;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--color-text-tertiary);
  font: inherit;
  font-size: var(--text-xs);
  font-weight: var(--weight-semibold);
  letter-spacing: 0.04em;
  text-align: start;
  text-transform: uppercase;
  cursor: pointer;
}

.app-rail__heading:hover {
  background: var(--color-bg-element);
  color: var(--color-text-secondary);
}

.app-rail__heading:focus-visible {
  outline: 2px solid var(--color-border-focus);
  outline-offset: 2px;
}

.app-rail__entries {
  min-inline-size: 0;
}

/* The reference keeps the product name on a row of its own, so the row - not
   the mark alone - is what a reader recognises, and the disclosure says the
   name can be changed from here rather than only displayed. */
.app-rail__product {
  gap: var(--space-2);
}

.app-rail__product-disclosure {
  flex: 0 0 auto;
  margin-inline-start: auto;
  color: var(--color-text-tertiary);
}

.app-rail__account-menu {
  min-inline-size: 0;
  flex: 1 1 auto;
}

.app-rail__account-menu :deep(.app-menu__trigger) {
  inline-size: 100%;
  justify-content: flex-start;
  padding: 0;
  border: 0;
  border-radius: var(--radius-sm);
  background: transparent;
}

.app-rail__account-menu :deep(.app-menu__trigger:hover) {
  background: var(--color-bg-element);
}

.app-rail__account-menu :deep(.app-menu__trigger:focus-visible) {
  outline: 2px solid var(--color-border-focus);
  outline-offset: 2px;
}
</style>
