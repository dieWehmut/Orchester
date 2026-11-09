<script setup lang="ts">
import type { AgentFleetSnapshotDto, SessionSummaryDto } from '@orchester/protokoll'
import { computed, ref, watch } from 'vue'
import { AppButton, AppDialog, AppInput } from '@orchester/design'
import ProjectRail from '../projects/ProjectRail.vue'
import ProjectEditor from '../projects/ProjectEditor.vue'
import SessionListItem from '../sessions/SessionListItem.vue'
import { useLibraryActions } from '../../composables/use-library-actions'
import { useAppStores } from '../../stores/app'
import { useShortcut } from '../../shortcuts'
import type { SessionAction } from '../sessions/session-actions'

import AppRail from './AppRail.vue'
import { AgentFleetPanel } from '../../features/agent-presence'
import SessionRail from '../sessions/SessionRail.vue'
import PinnedSessions from '../sessions/PinnedSessions.vue'
import { usePinnedSessions } from '../../composables/use-pinned-sessions'
import { useI18n } from '../../i18n'
import type { AgentFleetStoreStatus } from '../../stores/agent-fleet'
import type { AgentStatusSocketStatus } from '../../transport/agent-status-socket'
import type { SessionsStatus } from '../../stores/sessions'

const props = withDefaults(
  defineProps<{
  productName: string
  workspaceName: string | null
  /** Empty when the shell has no companion to hide. */
  companionLabel?: string
  /** How many approvals are waiting, for the header's bell. */
  attentionCount?: number
  searchLabel?: string
  attentionLabel?: string
  sessionStatus: SessionsStatus
  sessions: SessionSummaryDto[]
  selectedSessionId: string | null
  nextCursor: string | null
  sessionError: { message: string; retryable: boolean } | null
  agentStatus: AgentFleetStoreStatus
  agentStreamStatus: AgentStatusSocketStatus
  agentSnapshot: AgentFleetSnapshotDto | null
  agentError: string | null
  selectedAgentId: string | null
  /** The other destinations, kept in this column as the reference keeps them. */
  schedulesLabel?: string
  pluginsLabel?: string
  themeLabel?: string
  dark?: boolean
  companionVisible?: boolean
  }>(),
  {
    companionLabel: '',
    attentionCount: 0,
    searchLabel: 'Search',
    attentionLabel: '',
    schedulesLabel: '',
    pluginsLabel: '',
    themeLabel: '',
    dark: true,
    companionVisible: false,
  },
)

const emit = defineEmits<{
  selectSession: [id: string]
  refreshSessions: []
  loadMoreSessions: []
  newSession: []
  selectAgent: [id: string]
  openSettings: []
  toggleCompanion: []
  search: [query: string]
  openAttention: []
  openSchedules: []
  openPlugins: []
  toggleTheme: []
}>()

const { t } = useI18n()

/**
 * The header search is the rail's own filter, so the query lives here rather
 * than in either list: the field narrows what the column shows, and the header
 * that opens it is the column's own.
 */
const sessionQuery = ref('')

/**
 * The runs the reader keeps at the top, as the reference's sidebar opens.
 *
 * The pin is a reading preference and belongs to this column, so the state is
 * read here rather than routed through the view: nothing about a run changes
 * when it is pinned.
 */
const legacyPins = usePinnedSessions()
const controls = useLibraryActions()
const { library, nameDialog, name, validation, notice, projectOpen, project, share } = controls
const { sessions: sessionStore } = useAppStores()
const metadata = library.metadata
const destinations = computed(() => library.state.value.projects)
const sections = computed(() => library.state.value.sections)
const archived = ref(false)
const rows = computed(() => props.sessions.map(row => ({ ...row, title: metadata.value[row.id]?.title ?? row.title })))
const filteredRows = computed(() => {
  const needle = sessionQuery.value.trim().toLowerCase()
  return rows.value.filter(row => (!needle || [row.title, row.agent, row.model ?? ''].some(value => value.toLowerCase().includes(needle))))
})
const visibleRows = computed(() => filteredRows.value.filter(row => !metadata.value[row.id]?.archived))
const pinnedIds = computed(() => library.ready.value
  ? library.state.value.sessions.filter(row => row.pinned && !row.archived).map(row => row.id).reverse()
  : [...legacyPins.pinned.value])
const recentRows = computed(() => rows.value.filter(row => {
  const meta = metadata.value[row.id]
  return archived.value ? meta?.archived : !meta?.archived && !meta?.project_id && !meta?.section_id
}))
const sectionRows = (id: string) => visibleRows.value.filter(row => metadata.value[row.id]?.section_id === id && !pinnedIds.value.includes(row.id))
let migrated = false
const attemptedHydration = new Set<string>()
watch(() => [library.ready.value, props.sessions, props.sessionStatus, library.state.value.sessions.map(row => row.id).join(',')] as const, async () => {
  if (props.sessionStatus === 'refreshing') attemptedHydration.clear()
  if (!library.ready.value || props.sessionStatus !== 'ready') return
  if (!migrated && legacyPins.pinned.value.length) {
    migrated = true
    await sessionStore.ensureItems(legacyPins.pinned.value)
    const ids = legacyPins.pinned.value.filter(id => sessionStore.items.value.some(row => row.id === id) && !metadata.value[id])
    if (ids.length) {
      const saved = await library.mutate(state => {
        for (const id of [...ids].reverse()) if (!state.sessions.some(row => row.id === id)) state.sessions.push({ id, title: null, pinned: true, unread: false, archived: false, project_id: null, section_id: null })
      })
      if (!saved) migrated = false
    }
  }
  const missing = library.state.value.sessions.map(row => row.id).filter(id => !sessionStore.items.value.some(row => row.id === id) && !attemptedHydration.has(id))
  missing.forEach(id => attemptedHydration.add(id))
  if (missing.length) await sessionStore.ensureItems(missing)
}, { immediate: true })
function select(id: string): void {
  if (metadata.value[id]?.unread) void library.updateSession(id, row => { row.unread = false })
  emit('selectSession', id)
}
function togglePin(id: string): void { void controls.action({ id, action: 'pin' }) }
const shortcuts: { action: SessionAction['action']; label: 'rename' | 'pin' | 'markUnread' | 'copyLink' | 'copyDirectory' | 'archive'; keys: ('Ctrl' | 'Alt' | 'Shift' | string)[] }[] = [
  { action: 'rename', label: 'rename', keys: ['Ctrl', 'Alt', 'R'] },
  { action: 'pin', label: 'pin', keys: ['Ctrl', 'Alt', 'P'] },
  { action: 'unread', label: 'markUnread', keys: ['Ctrl', 'Shift', 'U'] },
  { action: 'copy-link', label: 'copyLink', keys: ['Ctrl', 'Alt', 'L'] },
  { action: 'copy-directory', label: 'copyDirectory', keys: ['Ctrl', 'Shift', 'C'] },
  { action: 'archive', label: 'archive', keys: ['Ctrl', 'Shift', 'A'] },
]
for (const shortcut of shortcuts) useShortcut({ id: `session.${shortcut.action}`, labelKey: `sessionMenu.${shortcut.label}`, groupKey: 'shortcuts.groups.sessions', keys: shortcut.keys }, () => {
  const focused = document.activeElement?.closest<HTMLElement>('[data-session-row]')?.dataset.sessionRow
  const id = focused ?? props.selectedSessionId
  if (id && !nameDialog.value && !projectOpen.value && !share.value) void controls.action({ id, action: shortcut.action })
})
</script>

<template>
  <AppRail
    class="workspace-sidebar"
    navigation-in-shell
    :product-name="props.productName"
    :workspace-name="props.workspaceName ?? t('workspace.projectFallback')"
    :new-session-label="t('sessions.newChat')"
    :projects-label="t('sessions.pinned')"
    :sessions-label="archived ? t('library.archived') : t('library.recent')"
    :fleet-label="t('agents.title')"
    :account-name="props.productName"
    :account-hint="t('account.localRuntime')"
    :settings-label="t('settings.title')"
    :companion-label="companionLabel"
    :search-label="searchLabel"
    :attention-label="attentionLabel"
    :attention-count="attentionCount"
    :schedules-label="props.schedulesLabel"
    :plugins-label="props.pluginsLabel"
    :theme-label="props.themeLabel"
    :dark="props.dark"
    :companion-visible="props.companionVisible"
    @new-session="$emit('newSession')"
    @open-settings="$emit('openSettings')"
    @toggle-companion="$emit('toggleCompanion')"
    @search="sessionQuery = $event"
    @open-attention="$emit('openAttention')"
    @open-schedules="$emit('openSchedules')"
    @open-plugins="$emit('openPlugins')"
    @toggle-theme="$emit('toggleTheme')"
  >
    <template #projects>
      <PinnedSessions
        :items="visibleRows"
        :pinned-ids="pinnedIds"
        :selected-id="props.selectedSessionId"
        :metadata="metadata" :projects="destinations" :sections="sections"
        @action="controls.action"
        @select="select"
        @toggle-pin="togglePin"
      />
    </template>

    <template #library>
      <ProjectRail :projects="destinations" :sessions="visibleRows.filter(row => !pinnedIds.includes(row.id))" :metadata="metadata" :sections="sections" :pinned-ids="pinnedIds" :selected-id="selectedSessionId" @create="controls.editProject(null)" @edit="controls.editProject($event)" @select="select" @action="controls.action" />
      <details v-for="section in sections" :key="section.id" class="library-section" open>
        <summary>{{ section.name }}</summary>
        <SessionListItem v-for="row in sectionRows(section.id)" :key="row.id" :session="row" :selected="row.id === selectedSessionId" :unread="metadata[row.id]?.unread" :section-id="section.id" :projects="destinations" :sections="sections" @select="select" @toggle-pin="togglePin" @action="controls.action" />
      </details>
    </template>
    <template #sessions>
      <SessionRail
        :status="props.sessionStatus"
        :items="recentRows"
        :selected-id="props.selectedSessionId"
        :metadata="metadata" :projects="destinations" :sections="sections"
        @action="controls.action"
        :next-cursor="props.nextCursor"
        :error="props.sessionError"
        :query="sessionQuery"
        :filter-source="rows.filter(row => archived ? metadata[row.id]?.archived : !metadata[row.id]?.archived)"
        :pinned-ids="archived ? [] : pinnedIds"
        @select="select"
        @refresh="$emit('refreshSessions')"
        @load-more="$emit('loadMoreSessions')"
        @toggle-pin="togglePin"
      />
      <AppButton size="sm" variant="ghost" class="archive-toggle" @click="archived = !archived">{{ archived ? t('library.showRecent') : t('library.showArchived') }}</AppButton>
      <p v-if="notice" role="status" class="library-notice">{{ notice }}</p>
      <div v-if="library.error.value" role="alert" class="library-error"><p>{{ t('library.failed') }}</p><AppButton size="sm" variant="ghost" @click="library.load">{{ t('library.retry') }}</AppButton></div>
    </template>

    <template #fleet>
      <AgentFleetPanel
        compact
        :status="props.agentStatus"
        :stream-status="props.agentStreamStatus"
        :snapshot="props.agentSnapshot"
        :error="props.agentError"
        :selected-agent-id="props.selectedAgentId"
        @select="$emit('selectAgent', $event)"
      />
    </template>
  </AppRail>
  <Teleport to="body">
    <ProjectEditor v-model:open="projectOpen" :project="project" :busy="library.busy.value" :error="library.error.value ? t('library.failed') : null" @save="controls.saveProject" @remove="controls.removeProject" />
    <AppDialog :open="!!nameDialog" :title="nameDialog?.kind === 'rename' ? t('library.rename') : t('library.newSection')" :close-label="t('library.close')" @update:open="!$event && (nameDialog = null)">
      <form @submit.prevent="controls.saveName"><label for="library-name">{{ t('library.name') }}</label><AppInput id="library-name" v-model="name" :disabled="library.busy.value" /><p v-if="validation || library.error.value" role="alert">{{ validation || t('library.failed') }}</p><div class="library-dialog-actions"><AppButton variant="ghost" @click="nameDialog = null">{{ t('library.cancel') }}</AppButton><AppButton type="submit" :busy="library.busy.value">{{ t('library.save') }}</AppButton></div></form>
    </AppDialog>
    <AppDialog :open="!!share" :title="t('library.share')" :description="t('library.shareDescription')" :close-label="t('library.close')" @update:open="!$event && (share = null)">
      <template v-if="share"><p>{{ share.title }}</p><div class="library-share-actions"><AppButton variant="secondary" @click="controls.copy(share.markdown)">{{ t('library.copyMarkdown') }}</AppButton><AppButton variant="secondary" @click="controls.copy(controls.localLink(share.id))">{{ t('library.copyLink') }}</AppButton><AppButton @click="controls.download">{{ t('library.download') }}</AppButton></div><p role="status">{{ notice }}</p></template>
    </AppDialog>
  </Teleport>
</template>

<style scoped>
.workspace-sidebar :deep(.app-rail__mark-slot) {
  display: none;
}

/* A group's empty note is a line in the column, not a panel in it: the
   reference's rail states "Threads" and leaves the space beneath it quiet, so
   the note loses the block padding that would make it read as a card. */
.workspace-sidebar :deep(.empty-state) {
  align-items: flex-start;
  justify-items: start;
  gap: var(--space-1);
  padding: var(--space-1) var(--space-2);
  text-align: start;
}

.workspace-sidebar :deep(.empty-state__title) {
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
  font-weight: var(--weight-normal);
}

.workspace-sidebar :deep(.empty-state__description) {
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
  line-height: var(--leading-snug);
}
</style>

<style scoped>
.workspace-sidebar {
  block-size: 100%;
}
</style>

<style scoped>
.library-section { margin-block: var(--space-2); }
.library-section summary { padding: var(--space-2); color: var(--color-text-tertiary); font-size: var(--text-xs); cursor: pointer; }
.library-notice, .library-error { padding: calc(var(--space-1) * 1.5) var(--space-2); font-size: 12px; }
.library-error { color: var(--color-status-error); }
.library-dialog-actions, .library-share-actions { display: flex; justify-content: flex-end; flex-wrap: wrap; gap: calc(var(--space-1) * 2.5); margin-top: var(--space-5); }
.archive-toggle { margin-top: var(--space-3); }
</style>
