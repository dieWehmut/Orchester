<script setup lang="ts">
import { computed, nextTick, ref } from 'vue'
import { AppMenu, Spinner } from '@orchester/design'
import { ArrowLeft, Check, ChevronDown, ChevronRight, Folder, FolderOpen, Search, SquarePen, X } from '@lucide/vue'
import { activityNames, activityProviders, type ExternalActivityProvider } from '../../../api/agent-activity'
import type { AgentActivity } from '../use-agent-activity'
import { useI18n } from '../../../i18n'
import AgentIcon from './AgentIcon.vue'
import ActivitySessionRow from './ActivitySessionRow.vue'
const props = defineProps<{ provider: ExternalActivityProvider; activity: AgentActivity }>()
const emit = defineEmits<{ provider: [value: ExternalActivityProvider]; close: []; newChat: []; select: [id: string] }>()
const { t } = useI18n()
const { sessions, selectedId, loading, prefs, isPinned, isUnread, projectKey, togglePin, toggleProject } = props.activity
const query = ref('')
const searching = ref(false)
const searchInput = ref<HTMLInputElement | null>(null)
const expanded = ref({ pinned: true, projects: false, recent: true })
const matches = computed(() => sessions.value.filter(row => `${row.title} ${row.project}`.toLocaleLowerCase().includes(query.value.trim().toLocaleLowerCase())))
const pinned = computed(() => matches.value.filter(isPinned))
const recent = computed(() => matches.value.filter(row => !isPinned(row)))
const projects = computed(() => {
  const groups = new Map<string, typeof sessions.value>()
  for (const row of matches.value) if (row.project) {
    const key = projectKey(row)
    groups.set(key, [...(groups.get(key) ?? []), row])
  }
  return [...groups].map(([key, rows]) => ({ key, title: rows[0]!.project, rows }))
})
function search(): void { searching.value = true; void nextTick(() => searchInput.value?.focus()) }
function closeSearch(): void { searching.value = false; query.value = '' }
</script>

<template>
  <aside class="activity-sidebar" data-activity-sidebar :aria-label="`${activityNames[provider]} ${t('activity.sessions')}`">
    <header class="activity-sidebar__provider">
      <button class="activity-sidebar__icon" type="button" :aria-label="t('activity.back')" @click="emit('close')"><ArrowLeft :size="17" /></button>
      <AppMenu :label="t('agents.title')" :items="activityProviders.map(id => ({ id, label: activityNames[id], checked: id === provider }))" @select="emit('provider', $event as ExternalActivityProvider)">
        <template #trigger><AgentIcon :icon-key="provider" /><strong>{{ activityNames[provider] }}</strong><ChevronDown :size="14" /></template>
        <template #item="{ item }"><AgentIcon :icon-key="item.id" /><span class="activity-sidebar__choice" :data-activity-provider="item.id">{{ item.label }}</span><Check :size="13" :style="{ visibility: item.checked ? 'visible' : 'hidden' }" aria-hidden="true" /></template>
      </AppMenu>
    </header>
    <div class="activity-sidebar__primary">
      <button type="button" class="activity-sidebar__new" data-rail-action="new-session" @click="emit('newChat')"><SquarePen :size="18" />{{ t('sessions.newChat') }}</button>
      <button type="button" class="activity-sidebar__icon" :aria-label="t('sessions.searchPlaceholder')" :aria-expanded="searching" @click="search"><Search :size="17" /></button>
    </div>
    <div v-if="searching" class="activity-sidebar__search"><input ref="searchInput" v-model="query" type="search" :aria-label="t('sessions.searchPlaceholder')" :placeholder="t('sessions.searchPlaceholder')" @keydown.escape="closeSearch" /><button type="button" :aria-label="t('activity.closeSearch')" @click="closeSearch"><X :size="15" /></button></div>
    <div class="activity-sidebar__scroll">
      <div v-if="loading && !sessions.length" class="activity-sidebar__empty"><Spinner :label="t('management.loading')" /></div>
      <template v-else>
        <section class="activity-sidebar__section" data-activity-section="pinned">
          <button type="button" class="activity-sidebar__heading" :aria-expanded="expanded.pinned" @click="expanded.pinned = !expanded.pinned">{{ t('sessions.pinned') }}<ChevronRight :size="13" :data-expanded="expanded.pinned" /></button>
          <div v-if="expanded.pinned"><ActivitySessionRow v-for="row in pinned" :key="row.id" :session="row" :selected="selectedId === row.id" pinned :unread="isUnread(row)" @select="emit('select', $event)" @pin="togglePin(row)" /><p v-if="!pinned.length" class="activity-sidebar__empty">{{ t('activity.noPinned') }}</p></div>
        </section>
        <section class="activity-sidebar__section" data-activity-section="projects">
          <button type="button" class="activity-sidebar__heading" :aria-expanded="expanded.projects" @click="expanded.projects = !expanded.projects">{{ t('activity.projects') }}<ChevronRight :size="13" :data-expanded="expanded.projects" /></button>
          <div v-if="expanded.projects || query">
            <div v-for="group in projects" :key="group.key" class="activity-sidebar__project">
              <button type="button" class="activity-sidebar__folder" :aria-expanded="prefs.projects.includes(group.key) || !!query" :data-activity-project="group.key" @click="toggleProject(group.key)"><FolderOpen v-if="prefs.projects.includes(group.key) || query" :size="18" /><Folder v-else :size="18" /><span>{{ group.title }}</span></button>
              <div v-if="prefs.projects.includes(group.key) || query"><ActivitySessionRow v-for="row in group.rows" :key="row.id" :session="row" :selected="selectedId === row.id" :pinned="isPinned(row)" :unread="isUnread(row)" nested @select="emit('select', $event)" @pin="togglePin(row)" /></div>
            </div>
          </div>
        </section>
        <section class="activity-sidebar__section" data-activity-section="recent">
          <button type="button" class="activity-sidebar__heading" :aria-expanded="expanded.recent" @click="expanded.recent = !expanded.recent">{{ t('activity.recent') }}<ChevronRight :size="13" :data-expanded="expanded.recent" /></button>
          <div v-if="expanded.recent"><ActivitySessionRow v-for="row in recent" :key="row.id" :session="row" :selected="selectedId === row.id" :pinned="false" :unread="isUnread(row)" @select="emit('select', $event)" @pin="togglePin(row)" /></div>
        </section>
        <p v-if="query && !matches.length" class="activity-sidebar__empty" role="status">{{ t('sessions.filterEmpty') }}</p>
      </template>
    </div>
  </aside>
</template>

<style scoped>
.activity-sidebar { --activity-selection: #2b2d2a; display: flex; flex-direction: column; block-size: 100%; min-block-size: 0; color: var(--color-text-primary); background: #1b1c19; }
:global([data-theme='light'] .activity-sidebar) { --activity-selection: var(--color-bg-element); background: var(--rail-surface); }
.activity-sidebar__provider { display: flex; align-items: center; gap: 4px; padding: 8px; min-block-size: 48px; }
.activity-sidebar__provider :deep(.app-menu) { flex: 1; min-inline-size: 0; }
.activity-sidebar__provider :deep(.app-menu__trigger) { display: flex; align-items: center; gap: 8px; inline-size: 100%; border: 0; background: transparent; font-size: 14px; }
.activity-sidebar__provider :deep(.agent-icon) { border: 0; background: transparent; inline-size: 22px; block-size: 22px; }
.activity-sidebar__provider :deep(.app-menu__item) { gap: 8px; }
.activity-sidebar__provider :deep(.app-menu__surface) { min-inline-size: 190px; max-inline-size: 100%; }
.activity-sidebar__choice { flex: 1; white-space: nowrap; }
.activity-sidebar__provider strong { flex: 1; text-align: start; font-weight: 500; }
.activity-sidebar__primary { display: flex; gap: 2px; padding: 0 8px 12px; }
.activity-sidebar__new { display: flex; flex: 1; align-items: center; gap: 10px; min-block-size: 36px; border: 0; border-radius: 10px; padding: 7px 10px; background: var(--activity-selection); color: inherit; font: inherit; font-size: 14px; cursor: pointer; text-align: start; }
.activity-sidebar__icon { display: grid; place-items: center; inline-size: 30px; block-size: 32px; border: 0; border-radius: 7px; background: transparent; color: var(--color-text-secondary); cursor: pointer; }
.activity-sidebar__icon:hover { background: var(--activity-selection); }
.activity-sidebar__scroll { flex: 1; min-block-size: 0; overflow-y: auto; overflow-x: hidden; scrollbar-width: thin; scrollbar-color: var(--color-border-base) transparent; padding: 0 8px 16px; }
.activity-sidebar__section + .activity-sidebar__section { margin-block-start: 20px; }
.activity-sidebar__heading { display: flex; align-items: center; gap: 6px; padding: 8px 10px; margin-block-end: 4px; background: none; border: 0; color: var(--color-text-tertiary); font: inherit; font-size: 13px; font-weight: 600; cursor: pointer; }
.activity-sidebar__heading svg { opacity: 0; }
.activity-sidebar__heading:hover svg, .activity-sidebar__heading:focus-visible svg, .activity-sidebar__heading[aria-expanded='false'] svg { opacity: 1; }
.activity-sidebar__heading svg[data-expanded='true'] { transform: rotate(90deg); }
.activity-sidebar__folder { display: flex; align-items: center; gap: 10px; inline-size: 100%; min-block-size: 36px; padding: 7px 10px; border: 0; border-radius: 8px; background: none; color: inherit; font: inherit; font-size: 14px; cursor: pointer; text-align: start; }
.activity-sidebar__folder span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.activity-sidebar__folder svg { flex-shrink: 0; }
.activity-sidebar__folder:hover { background: var(--activity-selection); }
.activity-sidebar__empty { margin: 0; padding: 4px 10px; font-size: 12px; color: var(--color-text-tertiary); }
.activity-sidebar__search { display: flex; align-items: center; margin: 0 10px 10px; border: 1px solid var(--color-border-base); border-radius: 8px; }
.activity-sidebar__search input { inline-size: 100%; min-inline-size: 0; padding: 8px; color: inherit; background: none; border: 0; font: inherit; font-size: 13px; }
.activity-sidebar__search button { display: grid; place-items: center; padding: 5px; border: 0; color: inherit; background: none; cursor: pointer; }
button:focus-visible, input:focus-visible { outline: 2px solid var(--color-border-focus); outline-offset: -2px; }
</style>
