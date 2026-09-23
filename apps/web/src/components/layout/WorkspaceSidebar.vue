<script setup lang="ts">
import type { AgentFleetSnapshotDto, SessionSummaryDto } from '@orchester/protokoll'
import { ref } from 'vue'

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
  }>(),
  { companionLabel: '', attentionCount: 0, searchLabel: 'Search', attentionLabel: '' },
)

defineEmits<{
  selectSession: [id: string]
  refreshSessions: []
  loadMoreSessions: []
  newSession: []
  selectAgent: [id: string]
  openSettings: []
  toggleCompanion: []
  search: [query: string]
  openAttention: []
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
const pinnedSessions = usePinnedSessions()
</script>

<template>
  <AppRail
    class="workspace-sidebar"
    :product-name="props.productName"
    :workspace-name="props.workspaceName ?? t('workspace.projectFallback')"
    :new-session-label="t('sessions.newChat')"
    :projects-label="t('sessions.pinned')"
    :sessions-label="t('sessions.railTitle')"
    :fleet-label="t('agents.title')"
    :account-name="props.productName"
    :account-hint="t('account.localRuntime')"
    :settings-label="t('settings.title')"
    :companion-label="companionLabel"
    :search-label="searchLabel"
    :attention-label="attentionLabel"
    :attention-count="attentionCount"
    @new-session="$emit('newSession')"
    @open-settings="$emit('openSettings')"
    @toggle-companion="$emit('toggleCompanion')"
    @search="sessionQuery = $event"
    @open-attention="$emit('openAttention')"
  >
    <template #projects>
      <PinnedSessions
        :items="props.sessions"
        :pinned-ids="pinnedSessions.pinned.value"
        :selected-id="props.selectedSessionId"
        @select="$emit('selectSession', $event)"
        @toggle-pin="pinnedSessions.toggle($event)"
      />
    </template>

    <template #sessions>
      <SessionRail
        :status="props.sessionStatus"
        :items="props.sessions"
        :selected-id="props.selectedSessionId"
        :next-cursor="props.nextCursor"
        :error="props.sessionError"
        :query="sessionQuery"
        :pinned-ids="pinnedSessions.pinned.value"
        @select="$emit('selectSession', $event)"
        @refresh="$emit('refreshSessions')"
        @load-more="$emit('loadMoreSessions')"
        @toggle-pin="pinnedSessions.toggle($event)"
      />
    </template>

    <template #fleet>
      <AgentFleetPanel
        :status="props.agentStatus"
        :stream-status="props.agentStreamStatus"
        :snapshot="props.agentSnapshot"
        :error="props.agentError"
        :selected-agent-id="props.selectedAgentId"
        @select="$emit('selectAgent', $event)"
      />
    </template>
  </AppRail>
</template>

<style scoped>
.workspace-sidebar {
  block-size: 100%;
}
</style>
