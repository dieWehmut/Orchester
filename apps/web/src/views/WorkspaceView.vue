<script setup lang="ts">
import InspectorDock from '../components/layout/InspectorDock.vue'
import WorkspaceResponsive from '../components/layout/WorkspaceResponsive.vue'
import WorkspaceSidebar from '../components/layout/WorkspaceSidebar.vue'
import SessionTranscript from '../components/sessions/SessionTranscript.vue'
import RunPanel from '../components/run/RunPanel.vue'
import { useI18n } from '../i18n'
import { useAppStores } from '../stores/app'
import { computed } from 'vue'

const { t } = useI18n()
const { sessions, run, agents, bootstrap, models } = useAppStores()
const runView = computed(() => run.view.value)
const runConnectionStatus = computed(() => run.connectionStatus.value)
const runProjectionStatus = computed(() => run.projectionStatus.value)
const runErrorMessage = computed(() => run.error.value?.message ?? null)
const conversationStarted = computed(() => run.conversationStarted.value)
const agentStatus = computed(() => agents.status)
const agentStreamStatus = computed(() => agents.streamStatus)
const agentSnapshot = computed(() => agents.snapshot)
const agentError = computed(() => agents.error?.message ?? null)
const workspaceName = computed(() => bootstrap.context.value?.workspace.name ?? null)
const modelCatalog = computed(() => models.catalog)
const modelStatus = computed(() => models.status)
const {
  status,
  detailStatus,
  items,
  nextCursor,
  selectedId,
  selected,
  error,
  detailError,
} = sessions

const runBusy = computed(() =>
  run.lifecycle.value === 'submitting' ||
  run.lifecycle.value === 'running' ||
  run.lifecycle.value === 'cancelling',
)

async function handleRunSubmit(prompt: string): Promise<void> {
  await run.submit(prompt)
}

async function handleRunCancel(): Promise<void> {
  await run.cancel()
}
</script>

<template>
  <WorkspaceResponsive
    data-testid="workspace-view"
    :sessions-title="t('sessions.title')"
    :inspector-title="t('inspector.label')"
    :controls-label="t('inspector.label')"
  >
    <template #sessions>
      <WorkspaceSidebar
        :session-status="status"
        :sessions="items"
        :selected-session-id="selectedId"
        :next-cursor="nextCursor"
        :session-error="error"
        :agent-status="agentStatus"
        :agent-stream-status="agentStreamStatus"
        :agent-snapshot="agentSnapshot"
        :agent-error="agentError"
        @select-session="sessions.select"
        @refresh-sessions="sessions.load"
        @load-more-sessions="sessions.loadMore"
        @new-session="sessions.select(null)"
      />
    </template>

    <RunPanel
      v-if="!selected"
      :view="runView"
      :connection-status="runConnectionStatus"
      :projection-status="runProjectionStatus"
      :error-message="runErrorMessage"
      :busy="runBusy"
      :conversation-started="conversationStarted"
      :workspace-name="workspaceName"
      :model-catalog="modelCatalog"
      :model-status="modelStatus"
      @submit="handleRunSubmit"
      @cancel="handleRunCancel"
    />
    <SessionTranscript v-else :status="detailStatus" :session="selected" :error="detailError" />

    <template #inspector>
      <InspectorDock />
    </template>
  </WorkspaceResponsive>
</template>
