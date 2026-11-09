<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
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
const { sessions, environments, environment, selectEnvironment, selectedId, loading, isPinned, isUnread, projectKey, togglePin } = props.activity
const query = ref('')
const searching = ref(false)
const searchInput = ref<HTMLInputElement | null>(null)
const expanded = ref({ pinned: true, projects: true })
const matches = computed(() => sessions.value.filter(row => `${row.title} ${row.cwd ?? ''} ${row.project} ${row.environment ?? ''}`.toLocaleLowerCase().includes(query.value.trim().toLocaleLowerCase())))
const pinned = computed(() => matches.value.filter(isPinned))
const collapsed = ref(new Set<string>())
const groupLimits = ref<Record<string, number>>({})
const isOpen = (key: string) => !!query.value || !collapsed.value.has(key)
function toggleGroup(key: string): void { if (collapsed.value.has(key)) collapsed.value.delete(key); else collapsed.value.add(key) }
watch(() => [props.provider, query.value, environment.value], () => { groupLimits.value = {} })
const projects = computed(() => {
  const groups = new Map<string, typeof sessions.value>()
  for (const row of matches.value) if (!isPinned(row)) {
    const key = projectKey(row)
    groups.set(key, [...(groups.get(key) ?? []), row])
  }
  return [...groups].map(([key, rows]) => ({ key, title: rows[0]!.cwd || rows[0]!.project || t('activity.ungrouped'), environment: rows[0]!.environment, rows }))
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
    <label v-if="environments.length > 1 || environment" class="activity-sidebar__environment">
      <span>{{ t('activity.environment') }}</span>
      <select :value="environment" data-activity-environment-filter @change="selectEnvironment(($event.target as HTMLSelectElement).value)">
        <option value="">{{ t('activity.allEnvironments') }}</option>
        <option v-for="source in environments" :key="source.label" :value="source.label">{{ source.label }} ({{ source.count }})</option>
      </select>
    </label>
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
              <button type="button" class="activity-sidebar__folder" :title="[group.title, group.environment].filter(Boolean).join(' · ')" :aria-expanded="isOpen(group.key)" :data-activity-project="group.key" @click="toggleGroup(group.key)"><FolderOpen v-if="isOpen(group.key)" :size="18" /><Folder v-else :size="18" /><span class="activity-sidebar__path">{{ group.title }}</span><span class="activity-sidebar__count" :aria-label="t('activity.sessionCount', { count: String(group.rows.length) })">{{ group.rows.length }}</span></button><p v-if="group.environment" class="activity-sidebar__group-environment">{{ group.environment }}</p>
              <div v-if="isOpen(group.key)"><ActivitySessionRow v-for="row in group.rows.slice(0, groupLimits[group.key] ?? 50)" :key="row.id" :session="row" :selected="selectedId === row.id" :pinned="isPinned(row)" :unread="isUnread(row)" nested @select="emit('select', $event)" @pin="togglePin(row)" /><button v-if="group.rows.length > (groupLimits[group.key] ?? 50)" type="button" class="activity-sidebar__heading" data-activity-more @click="groupLimits[group.key] = (groupLimits[group.key] ?? 50) + 50">{{ t('activity.showMore', { count: String(group.rows.length - (groupLimits[group.key] ?? 50)) }) }}</button></div>
            </div>
          </div>
        </section>
        <p v-if="query && !matches.length" class="activity-sidebar__empty" role="status">{{ t('sessions.filterEmpty') }}</p>
      </template>
    </div>
  </aside>
</template>

<style scoped>
.activity-sidebar { --activity-selection: var(--color-bg-element); display: flex; flex-direction: column; block-size: 100%; min-block-size: 0; color: var(--color-text-primary); background: var(--rail-surface, var(--color-bg-surface)); }
:global([data-theme='light'] .activity-sidebar) { --activity-selection: var(--color-bg-element); background: var(--rail-surface); }
.activity-sidebar__provider { display: flex; align-items: center; gap: var(--space-1); padding: var(--space-2); min-block-size: 48px; }
.activity-sidebar__provider :deep(.app-menu) { flex: 1; min-inline-size: 0; }
.activity-sidebar__provider :deep(.app-menu__trigger) { display: flex; align-items: center; gap: var(--space-2); inline-size: 100%; border: 0; background: transparent; font-size: 14px; }
.activity-sidebar__provider :deep(.agent-icon) { border: 0; background: transparent; inline-size: 22px; block-size: 22px; }
.activity-sidebar__provider :deep(.app-menu__item) { gap: var(--space-2); }
.activity-sidebar__provider :deep(.app-menu__surface) { min-inline-size: 190px; max-inline-size: 100%; }
.activity-sidebar__choice { flex: 1; white-space: nowrap; }
.activity-sidebar__environment { display: grid; gap: var(--space-1); padding: 0 var(--space-3) var(--space-3); color: var(--color-text-tertiary); font-size: 12px; }
.activity-sidebar__environment select { inline-size: 100%; min-inline-size: 0; padding: var(--space-2); border: 1px solid var(--color-border-base); border-radius: 8px; background: var(--color-bg-surface); color: var(--color-text-secondary); font: inherit; }
.activity-sidebar__provider strong { flex: 1; text-align: start; font-weight: 500; }
.activity-sidebar__primary { display: flex; gap: 2px; padding: 0 var(--space-2) var(--space-3); }
.activity-sidebar__new { display: flex; flex: 1; align-items: center; gap: calc(var(--space-1) * 2.5); min-block-size: 36px; border: 0; border-radius: 10px; padding: calc(var(--space-1) * 1.75) calc(var(--space-1) * 2.5); background: var(--activity-selection); color: inherit; font: inherit; font-size: 14px; cursor: pointer; text-align: start; }
.activity-sidebar__icon { display: grid; place-items: center; inline-size: 30px; block-size: 32px; border: 0; border-radius: 7px; background: transparent; color: var(--color-text-secondary); cursor: pointer; }
.activity-sidebar__icon:hover { background: var(--activity-selection); }
.activity-sidebar__scroll { flex: 1; min-block-size: 0; overflow-y: auto; overflow-x: hidden; scrollbar-width: thin; scrollbar-color: var(--color-border-base) transparent; padding: 0 var(--space-2) var(--space-4); }
.activity-sidebar__section + .activity-sidebar__section { margin-block-start: var(--space-5); }
.activity-sidebar__heading { display: flex; align-items: center; gap: calc(var(--space-1) * 1.5); padding: var(--space-2) calc(var(--space-1) * 2.5); margin-block-end: var(--space-1); background: none; border: 0; color: var(--color-text-tertiary); font: inherit; font-size: 13px; font-weight: 600; cursor: pointer; }
.activity-sidebar__heading svg { opacity: 0; }
.activity-sidebar__heading:hover svg, .activity-sidebar__heading:focus-visible svg, .activity-sidebar__heading[aria-expanded='false'] svg { opacity: 1; }
.activity-sidebar__heading svg[data-expanded='true'] { transform: rotate(90deg); }
.activity-sidebar__folder { display: flex; align-items: center; gap: calc(var(--space-1) * 2.5); inline-size: 100%; min-block-size: 36px; padding: calc(var(--space-1) * 1.75) calc(var(--space-1) * 2.5); border: 0; border-radius: 8px; background: none; color: inherit; font: inherit; font-size: 14px; cursor: pointer; text-align: start; }
.activity-sidebar__folder span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.activity-sidebar__folder svg { flex-shrink: 0; }
.activity-sidebar__folder:hover { background: var(--activity-selection); }
.activity-sidebar__empty { margin: 0; padding: var(--space-1) calc(var(--space-1) * 2.5); font-size: 12px; color: var(--color-text-tertiary); }
.activity-sidebar__search { display: flex; align-items: center; margin: 0 calc(var(--space-1) * 2.5) calc(var(--space-1) * 2.5); border: 1px solid var(--color-border-base); border-radius: 8px; }
.activity-sidebar__search input { inline-size: 100%; min-inline-size: 0; padding: var(--space-2); color: inherit; background: none; border: 0; font: inherit; font-size: 13px; }
.activity-sidebar__search button { display: grid; place-items: center; padding: calc(var(--space-1) * 1.25); border: 0; color: inherit; background: none; cursor: pointer; }
button:focus-visible, input:focus-visible { outline: 2px solid var(--color-border-focus); outline-offset: -2px; }
.activity-sidebar__path { flex: 1; min-inline-size: 0; font-family: var(--font-mono, monospace); font-size: 12px; }
.activity-sidebar__count { flex-shrink: 0; color: var(--color-text-tertiary); font-variant-numeric: tabular-nums; }
.activity-sidebar__group-environment { margin: 0 10px 4px 38px; font-size: 11px; color: var(--color-text-tertiary); overflow-wrap: anywhere; }
.activity-sidebar__project + .activity-sidebar__project { margin-top: 14px; }
</style>
