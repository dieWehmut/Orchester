<script setup lang="ts">
import { ref } from 'vue'
import { Folder, ChevronRight, Ellipsis, Plus } from '@lucide/vue'
import type { SessionSummaryDto } from '@orchester/protokoll'
import SessionListItem from '../sessions/SessionListItem.vue'
import type { SessionAction } from '../sessions/session-actions'
import type { LocalProject } from './project-folders'
import { useI18n } from '../../i18n'
const props = withDefaults(defineProps<{ projects: readonly LocalProject[]; sessions: readonly SessionSummaryDto[]; metadata: Record<string, { project_id: string | null; section_id: string | null; unread: boolean; archived: boolean }>; selectedId: string | null; sections?: readonly {id: string; name: string}[]; pinnedIds?: readonly string[] }>(), { sections: () => [], pinnedIds: () => [] })
const emit = defineEmits<{ edit: [string]; create: []; select: [string]; action: [SessionAction] }>()
const collapsed = ref(new Set<string>())
const { t } = useI18n()
function toggle(id: string) { const next = new Set(collapsed.value); if (next.has(id)) next.delete(id); else next.add(id); collapsed.value = next }
function entries(id: string) { return props.sessions.filter(session => props.metadata[session.id]?.project_id === id && !props.metadata[session.id]?.archived) }
</script>
<template>
  <section class="project-rail" :aria-label="t('projectLibrary.projects')">
    <div class="rail-title"><span>{{ t('projectLibrary.projects') }}</span><button type="button" :aria-label="t('projectLibrary.create')" @click="emit('create')"><Plus :size="15" /></button></div>
    <section v-for="project in projects" :key="project.id" class="project-group">
      <div class="project-heading"><button class="project-disclosure" type="button" :aria-expanded="!collapsed.has(project.id)" @click="toggle(project.id)"><ChevronRight :size="13" :class="{ expanded: !collapsed.has(project.id) }" /><Folder :size="17" /><span>{{ project.name }}</span></button><button class="project-more" type="button" :aria-label="t('projectLibrary.editLabel') + project.name" @click="emit('edit', project.id)"><Ellipsis :size="17" /></button></div>
      <div v-if="!collapsed.has(project.id)" class="project-sessions"><SessionListItem v-for="session in entries(project.id)" :key="session.id" :session="session" :selected="selectedId === session.id" :unread="metadata[session.id]?.unread ?? false" :archived="metadata[session.id]?.archived ?? false" :project-id="metadata[session.id]?.project_id ?? null" :section-id="metadata[session.id]?.section_id ?? null" :projects="projects" :sections="sections" :pinned="pinnedIds.includes(session.id)" @toggle-pin="emit('action', { id: $event, action: 'pin' })" @select="emit('select', $event)" @action="emit('action', $event)" /><p v-if="!entries(project.id).length">{{ t('projectLibrary.empty') }}</p></div>
    </section>
  </section>
</template>
<style scoped>
.project-rail { display: grid; gap: calc(var(--space-1) * 1.5); padding-block: var(--space-2); }
.rail-title, .project-heading { display: flex; align-items: center; }
.rail-title { justify-content: space-between; color: var(--color-text-tertiary); font-size: 12px; padding-inline: calc(var(--space-1) * 2.5) 2px; }
button { background: transparent; border: 0; color: inherit; font: inherit; cursor: pointer; min-height: 32px; }
.rail-title button, .project-more { display: grid; place-items: center; min-width: 32px; border-radius: 8px; }
.project-disclosure { display: flex; align-items: center; gap: calc(var(--space-1) * 1.75); flex: 1; min-width: 0; text-align: start; padding-inline: var(--space-1); font-size: 13px; color: var(--color-text-secondary); }
.project-disclosure svg { flex-shrink: 0; }
.project-disclosure span { overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
.expanded { transform: rotate(90deg); }
.project-heading:hover, button:hover { background: var(--workspace-hover, var(--color-bg-element)); border-radius: 8px; }
.project-more { color: var(--color-text-tertiary); }
.project-sessions { padding-inline-start: var(--space-3); }
.project-sessions p { color: var(--color-text-tertiary); font-size: 12px; padding: var(--space-1) calc(var(--space-1) * 2.5); }
</style>
