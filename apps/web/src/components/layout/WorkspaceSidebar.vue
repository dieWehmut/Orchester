<script setup lang="ts">
import type { AgentFleetSnapshotDto, SessionSummaryDto } from '@orchester/protokoll'

import AgentFleetPanel from '../agents/AgentFleetPanel.vue'
import SessionRail from '../sessions/SessionRail.vue'
import type { AgentFleetStoreStatus } from '../../stores/agent-fleet'
import type { AgentStatusSocketStatus } from '../../transport/agent-status-socket'
import type { SessionsStatus } from '../../stores/sessions'

defineProps<{
  sessionStatus: SessionsStatus
  sessions: SessionSummaryDto[]
  selectedSessionId: string | null
  nextCursor: string | null
  sessionError: { message: string; retryable: boolean } | null
  agentStatus: AgentFleetStoreStatus
  agentStreamStatus: AgentStatusSocketStatus
  agentSnapshot: AgentFleetSnapshotDto | null
  agentError: string | null
}>()

defineEmits<{
  selectSession: [id: string]
  refreshSessions: []
  loadMoreSessions: []
  newSession: []
  selectAgent: [id: string]
}>()
</script>

<template>
  <div class="workspace-sidebar">
    <SessionRail
      class="workspace-sidebar__sessions"
      :status="sessionStatus"
      :items="sessions"
      :selected-id="selectedSessionId"
      :next-cursor="nextCursor"
      :error="sessionError"
      @select="$emit('selectSession', $event)"
      @refresh="$emit('refreshSessions')"
      @load-more="$emit('loadMoreSessions')"
      @new-session="$emit('newSession')"
    />
    <AgentFleetPanel
      class="workspace-sidebar__agents"
      :status="agentStatus"
      :stream-status="agentStreamStatus"
      :snapshot="agentSnapshot"
      :error="agentError"
      @select="$emit('selectAgent', $event)"
    />
  </div>
</template>

<style scoped>
.workspace-sidebar {
  display: grid;
  grid-template-rows: minmax(0, 1fr) auto;
  min-block-size: 100%;
  background: var(--color-bg-surface);
}

.workspace-sidebar__sessions {
  min-block-size: 0;
  overflow: auto;
}

.workspace-sidebar__agents {
  max-block-size: min(45vh, 30rem);
  overflow: auto;
  border-block-start: 1px solid var(--color-border-base);
  background: color-mix(in srgb, var(--color-bg-surface) 94%, var(--color-bg-base));
}
</style>
