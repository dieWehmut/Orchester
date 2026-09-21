<script setup lang="ts">
import {
  Bell,
  ChevronDown,
  ChevronRight,
  CircleUser,
  Search,
  Settings,
  SquarePen,
} from '@lucide/vue'
import { computed, nextTick, ref } from 'vue'

import { AppButton, AppMenu, IconButton, type AppMenuItem } from '@orchester/design'

import mark from '../../assets/orchester-mark.png'
import { formatShortcut, shortcutRegistry } from '../../shortcuts'
import { readDocumentPlatform, readSystemPlatform } from '@orchester/design'

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

/**
 * Whether the product row has folded the lists it governs.
 *
 * The reference's product row is the parent of everything under it, so its
 * disclosure is the one control that clears the whole column in a single
 * gesture. The per-section headings below still fold one list at a time; this
 * is the coarse version of the same intent, which is what a narrow rail wants
 * when the reader is working in the transcript rather than browsing work.
 */
const railFolded = ref(false)

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
    /** The accessible name the header's search action carries. */
    searchLabel?: string
    /** What the header's bell says is waiting, when anything is. */
    attentionLabel?: string
    /** How many items are waiting; the bell carries the count. */
    attentionCount?: number
  }>(),
  {
    workspaceName: null,
    accountName: null,
    accountHint: null,
    settingsLabel: 'Settings',
    companionLabel: '',
    searchLabel: 'Search',
    attentionLabel: '',
    attentionCount: 0,
  },
)

const emit = defineEmits<{
  newSession: []
  openSettings: []
  toggleCompanion: []
  search: [query: string]
  openAttention: []
}>()

/**
 * Whether the header's search field is open.
 *
 * The reference shows a search glyph rather than a permanent field, because the
 * header row belongs to the product name and only lends its trailing space to
 * the field while the reader is searching.
 */
const searching = ref(false)
const query = ref('')
const searchField = ref<HTMLInputElement | null>(null)

function openSearch(): void {
  searching.value = true
  // The field appears where the reader just clicked, so it takes the focus
  // itself: an action that opens a text box the reader then has to find and
  // click again is a box that appeared near a button, not a search action.
  void nextTick(() => searchField.value?.focus())
  if (query.value.length > 0) emit('search', query.value)
}

function closeSearch(): void {
  searching.value = false
  query.value = ''
  emit('search', '')
}

function handleQuery(value: string): void {
  query.value = value
  emit('search', value)
}

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
  ([
    props.companionLabel
      ? { id: 'companion', label: props.companionLabel, hint: companionHint.value }
      : null,
    {
      id: 'settings',
      label: props.settingsLabel,
      // The reference prints the chord on the row, and printing it from the
      // registry is what keeps it true after a rebinding.
      hint: settingsHint.value,
    },
  ] as (AppMenuItem | null)[]).filter((item): item is AppMenuItem => item !== null),
)

/**
 * The chord that opens settings, as this platform spells it.
 *
 * Read off the live registry rather than written out: the settings editor lets
 * the reader rebind it, and a menu that kept the default would be teaching a
 * key that no longer does anything.
 */
const settingsHint = computed(() => {
  const keys = shortcutRegistry.effectiveKeys('settings.open')
  if (keys === undefined) return undefined
  return formatShortcut(keys, readDocumentPlatform() ?? readSystemPlatform())
})

/**
 * The chord that toggles the companion, read off the same registry.
 *
 * The reference teaches this chord on the row that performs it, so it has to
 * come from the live binding: a hard-coded one would keep teaching the default
 * after the reader rebound it.
 */
const companionHint = computed(() => {
  const keys = shortcutRegistry.effectiveKeys('companion.toggle')
  if (keys === undefined) return undefined
  return formatShortcut(keys, readDocumentPlatform() ?? readSystemPlatform())
})

function chooseAccountItem(id: string): void {
  if (id === 'settings') emit('openSettings')
  else if (id === 'companion') emit('toggleCompanion')
}
</script>

<template>
  <div class="app-rail" data-app-rail>
    <div class="app-rail__section app-rail__header" data-rail-section="brand">
      <button
        class="app-rail__product"
        type="button"
        data-rail-product
        :aria-expanded="!railFolded"
        @click="railFolded = !railFolded"
      >
        <span class="app-rail__mark-slot" aria-hidden="true">
          <img class="app-rail__mark" data-rail-mark :src="mark" alt="" draggable="false" />
        </span>
        <span class="app-rail__identity">
          <strong>{{ productName }}</strong>
          <span v-if="workspaceName">{{ workspaceName }}</span>
        </span>
        <ChevronDown
          v-if="!railFolded"
          class="app-rail__product-disclosure"
          :size="16"
          aria-hidden="true"
          data-rail-product-disclosure
        />
        <ChevronRight
          v-else
          class="app-rail__product-disclosure"
          :size="16"
          aria-hidden="true"
          data-rail-product-disclosure
        />
      </button>

      <!--
        The header's own actions. The reference keeps both glyphs on the
        product's row rather than inside the lists, because they act on the
        column the row names: search narrows what is underneath, and the bell
        reports what is waiting there. They are siblings of the product button
        rather than children of it, because a button inside a button is not a
        control a browser will let the reader press.
      -->
      <IconButton
        :label="searchLabel"
        data-rail-action="search"
        @click="openSearch"
      >
        <Search :size="15" aria-hidden="true" />
      </IconButton>
      <IconButton
        v-if="attentionCount > 0"
        :label="`${attentionLabel} (${attentionCount})`"
        data-rail-action="attention"
        @click="$emit('openAttention')"
      >
        <Bell :size="15" aria-hidden="true" />
        <span class="app-rail__attention-count" aria-hidden="true">{{ attentionCount }}</span>
      </IconButton>
    </div>

    <div v-if="searching" class="app-rail__search" data-rail-search>
      <Search :size="15" aria-hidden="true" />
      <input
        ref="searchField"
        :value="query"
        type="search"
        :placeholder="searchLabel"
        :aria-label="searchLabel"
        @input="handleQuery(($event.target as HTMLInputElement).value)"
        @keydown.escape="closeSearch"
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
      v-if="!railFolded"
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
      v-if="!railFolded"
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
      v-if="!railFolded"
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
  /* A column rather than a fixed row count: the product row can fold the lists
     out of the tree, and a grid whose tracks were named per child would slide
     the account row up into a list's track when that happened. */
  display: flex;
  flex-direction: column;
  block-size: 100%;
  min-block-size: 0;
  background: var(--rail-surface);
  backdrop-filter: var(--rail-blur);
}

.app-rail__section {
  min-inline-size: 0;
  padding: var(--space-3);
}

/* The lists share whatever is left between the product row and the account
   row, and each one scrolls on its own rather than the column scrolling. */
.app-rail__section--scroll,
.app-rail__section--footer {
  flex: 1 1 0;
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
  margin-block-start: auto;
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
  border-block-end: 1px solid var(--color-border-base);
}

/* The header's own row. The product button takes the space and the actions
   keep their own, so a long workspace name truncates rather than pushing the
   search glyph off the row. */
.app-rail__header {
  gap: var(--space-2);
}

.app-rail__header .app-rail__product {
  flex: 1 1 auto;
}

.app-rail__header > .icon-button {
  flex: 0 0 auto;
}

/* The count rides the bell's corner rather than sitting beside it: the
   reference has one glyph whose meaning is the number on it, and a number in
   the row would read as a label for the control next to it. */
.app-rail__attention-count {
  position: absolute;
  inset-block-start: 2px;
  inset-inline-end: 2px;
  min-inline-size: 0.875rem;
  padding-inline: 0.1875rem;
  border-radius: var(--radius-full, 999px);
  background: var(--color-accent);
  color: var(--color-accent-contrast);
  font-size: 0.625rem;
  font-weight: var(--weight-semibold);
  line-height: 0.875rem;
  text-align: center;
}

.app-rail__header > .icon-button {
  position: relative;
}

/* The filter field sits under the header row it belongs to and spans the
   column, because it narrows the lists below rather than searching one. */
.app-rail__search {
  display: flex;
  min-inline-size: 0;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  border-block-end: 1px solid var(--color-border-base);
  color: var(--color-text-tertiary);
}

.app-rail__search input {
  min-inline-size: 0;
  flex: 1 1 auto;
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--color-text-primary);
  font: inherit;
  font-size: var(--text-sm);
}

.app-rail__search input:focus-visible {
  outline: none;
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
  /* The name's column shrinks but does not grow, which is what keeps the
     disclosure beside the name - as the reference draws it - rather than out
     at the row's trailing edge next to the header's own actions. */
  flex: 0 1 auto;
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
  display: flex;
  inline-size: 100%;
  min-block-size: var(--hit-target-min, 32px);
  align-items: center;
  gap: var(--space-2);
  border: 0;
  background: transparent;
  color: inherit;
  font: inherit;
  text-align: start;
  cursor: pointer;
}

.app-rail__product:hover {
  background: var(--color-bg-element);
}

.app-rail__product:focus-visible {
  outline: 2px solid var(--color-border-focus);
  outline-offset: -2px;
}

.app-rail__product-disclosure {
  flex: 0 0 auto;
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
