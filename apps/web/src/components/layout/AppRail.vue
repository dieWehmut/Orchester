<script setup lang="ts">
import { Bell, ChevronDown, ChevronRight, Search, SquarePen } from '@lucide/vue'
import { nextTick, ref } from 'vue'

withDefaults(defineProps<{
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
  searchLabel?: string
  attentionLabel?: string
  attentionCount?: number
}>(), { searchLabel: 'Search', attentionLabel: '', attentionCount: 0 })
const emit = defineEmits<{
  newSession: []
  openSettings: []
  toggleCompanion: []
  search: [query: string]
  openAttention: []
}>()
const collapsed = ref<Record<string, boolean>>({ pinned: false, projects: false, agents: false })
const searching = ref(false)
const query = ref('')
const searchField = ref<HTMLInputElement | null>(null)
const searchButton = ref<HTMLButtonElement | null>(null)
function openSearch(): void {
  searching.value = true
  void nextTick(() => searchField.value?.focus())
}
function closeSearch(): void {
  searching.value = false
  query.value = ''
  emit('search', '')
  void nextTick(() => searchButton.value?.focus())
}
function handleQuery(value: string): void { query.value = value; emit('search', value) }
</script>

<template>
  <div class="app-rail" data-app-rail>
    <div class="app-rail__primary" data-rail-section="primary">
      <button class="app-rail__row" type="button" data-rail-row="new-session" data-rail-action="new-session" @click="emit('newSession')">
        <SquarePen :size="18" aria-hidden="true" /><span>{{ newSessionLabel }}</span>
      </button>
      <button ref="searchButton" class="app-rail__icon" type="button" :aria-label="searchLabel" :aria-expanded="searching" data-rail-action="search" @click="openSearch"><Search :size="17" aria-hidden="true" /></button>
      <button v-if="attentionCount > 0" class="app-rail__icon" type="button" :aria-label="attentionLabel + ' (' + attentionCount + ')'" data-rail-action="attention" @click="emit('openAttention')"><Bell :size="17" aria-hidden="true" /><span class="app-rail__attention-count" aria-hidden="true">{{ attentionCount }}</span></button>
    </div>
    <div v-if="searching" class="app-rail__search" data-rail-search>
      <Search :size="15" aria-hidden="true" />
      <input ref="searchField" :value="query" type="search" :placeholder="searchLabel" :aria-label="searchLabel" @input="handleQuery(($event.target as HTMLInputElement).value)" @keydown.escape="closeSearch" />
    </div>
    <div class="app-rail__scroll" data-rail-scroll>
      <div class="app-rail__section" data-rail-section="projects" data-rail-heading="pinned">
        <button class="app-rail__heading" type="button" data-rail-disclosure="pinned" :aria-expanded="!collapsed.pinned" @click="collapsed.pinned = !collapsed.pinned">
          <ChevronRight v-if="collapsed.pinned" :size="13" aria-hidden="true" /><ChevronDown v-else :size="13" aria-hidden="true" />{{ projectsLabel }}
        </button>
        <div v-if="!collapsed.pinned" class="app-rail__entries"><slot name="projects" /></div>
      </div>
      <div class="app-rail__section" data-rail-section="fleet" data-rail-heading="agents">
        <button class="app-rail__heading" type="button" data-rail-disclosure="agents" :aria-expanded="!collapsed.agents" @click="collapsed.agents = !collapsed.agents">
          <ChevronRight v-if="collapsed.agents" :size="13" aria-hidden="true" /><ChevronDown v-else :size="13" aria-hidden="true" />{{ fleetLabel }}
        </button>
        <div v-if="!collapsed.agents" class="app-rail__entries"><slot name="fleet" /></div>
      </div>
      <div class="app-rail__section" data-rail-section="sessions" data-rail-heading="projects">
        <button class="app-rail__heading" type="button" data-rail-disclosure="projects" :aria-expanded="!collapsed.projects" @click="collapsed.projects = !collapsed.projects">
          <ChevronRight v-if="collapsed.projects" :size="13" aria-hidden="true" /><ChevronDown v-else :size="13" aria-hidden="true" />{{ sessionsLabel }}
        </button>
        <div v-if="!collapsed.projects" class="app-rail__entries"><slot name="sessions" /></div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.app-rail { display: flex; flex-direction: column; block-size: 100%; min-block-size: 0; background: var(--rail-surface); backdrop-filter: var(--rail-blur); }
.app-rail__primary { display: flex; align-items: center; gap: 2px; padding: var(--space-2); }
.app-rail__row { display: flex; flex: 1; align-items: center; gap: var(--space-3); min-inline-size: 0; min-block-size: 38px; padding: var(--space-2) var(--space-3); border: 0; border-radius: 10px; background: var(--workspace-hover, var(--color-bg-element)); color: var(--color-text-primary); font: inherit; font-size: var(--text-sm); cursor: pointer; text-align: start; }
.app-rail__icon { display: grid; position: relative; place-items: center; min-inline-size: 32px; min-block-size: 32px; padding: 0; border: 0; border-radius: 8px; background: transparent; color: var(--color-text-secondary); cursor: pointer; }
.app-rail__scroll { flex: 1; min-block-size: 0; overflow-y: auto; overflow-x: hidden; scrollbar-width: thin; scrollbar-color: var(--workspace-seam, var(--color-border-subtle)) transparent; }
.app-rail__section { min-inline-size: 0; padding: var(--space-2); }
.app-rail__heading { display: flex; align-items: center; gap: var(--space-2); inline-size: 100%; min-block-size: 32px; padding: var(--space-2); border: 0; border-radius: 6px; background: transparent; color: var(--color-text-tertiary); font: inherit; font-size: var(--text-xs); text-align: start; cursor: pointer; }
.app-rail__entries { min-inline-size: 0; }
.app-rail__icon:hover, .app-rail__heading:hover, .app-rail__row:hover { background: var(--workspace-hover, var(--color-bg-element)); color: var(--color-text-primary); }
.app-rail__heading:focus-visible, .app-rail__row:focus-visible, .app-rail__icon:focus-visible { outline: 2px solid var(--color-border-focus); outline-offset: -2px; }
.app-rail__search { display: flex; align-items: center; gap: var(--space-2); margin: 0 var(--space-2) var(--space-2); padding: var(--space-2); border: 1px solid var(--color-border-base); border-radius: 8px; color: var(--color-text-tertiary); }
.app-rail__search:focus-within { border-color: var(--color-border-focus); }
.app-rail__search input { inline-size: 100%; min-inline-size: 0; border: 0; outline: none; background: transparent; color: var(--color-text-primary); font: inherit; font-size: var(--text-sm); }
.app-rail__attention-count { position: absolute; inset-block-start: 0; inset-inline-end: 0; min-inline-size: 14px; border-radius: 8px; background: var(--color-accent); color: var(--color-bg-app); font-size: 9px; line-height: 14px; }
</style>
