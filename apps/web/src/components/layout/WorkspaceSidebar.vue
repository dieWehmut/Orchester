<script setup lang="ts">
import type { AgentFleetSnapshotDto, SessionSummaryDto } from '@orchester/protokoll'

import AppRail from './AppRail.vue'
import { AgentFleetPanel } from '../../features/agent-presence'
import SessionRail from '../sessions/SessionRail.vue'
import ProjectList from '../sessions/ProjectList.vue'
import { useI18n } from '../../i18n'
import type { AgentFleetStoreStatus } from '../../stores/agent-fleet'
import type { AgentStatusSocketStatus } from '../../transport/agent-status-socket'
import type { SessionsStatus } from '../../stores/sessions'

const props = defineProps<{
  productName: string
  workspaceName: string | null
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
}>()

defineEmits<{
  selectSession: [id: string]
  refreshSessions: []
  loadMoreSessions: []
  newSession: []
  selectAgent: [id: string]
}>()

const { t } = useI18n()
</script>

<template>
  <AppRail
    class="workspace-sidebar"
    :product-name="props.productName"
    :workspace-name="props.workspaceName"
    :new-session-label="t('sessions.newChat')"
    :projects-label="t('workspace.projects')"
    :sessions-label="t('sessions.railTitle')"
    :fleet-label="t('agents.title')"
    @new-session="$emit('newSession')"
  >
    <template #projects>
      <ProjectList
        :workspace-name="props.workspaceName"
        :fallback-label="t('workspace.projectFallback')"
      />
    </template>

    <template #sessions>
      <SessionRail
        :status="props.sessionStatus"
        :items="props.sessions"
        :selected-id="props.selectedSessionId"
        :next-cursor="props.nextCursor"
        :error="props.sessionError"
        @select="$emit('selectSession', $event)"
        @refresh="$emit('refreshSessions')"
        @load-more="$emit('loadMoreSessions')"
        @new-session="$emit('newSession')"
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
