<script setup lang="ts">
import { AgentDetails, agentActivityMessageKey } from '../features/agent-presence'
import ChangeInspector from '../components/changes/ChangeInspector.vue'
import { summarizeFileChanges } from '../components/changes/change-summary'
import InspectorDock from '../components/layout/InspectorDock.vue'
import type { InspectorTab } from '../components/layout/inspector-tabs'
import WorkspaceResponsive from '../components/layout/WorkspaceResponsive.vue'
import WorkspaceSidebar from '../components/layout/WorkspaceSidebar.vue'
import ThreadBar from '../components/layout/ThreadBar.vue'
import SessionTranscript from '../components/sessions/SessionTranscript.vue'
import RunPanel from '../components/run/RunPanel.vue'
import { useI18n } from '../i18n'
import { useAppStores } from '../stores/app'
import { computed, inject, ref } from 'vue'
import { routerKey } from 'vue-router'

const { t } = useI18n()
const appRouter = inject(routerKey, null)
const { sessions, run, agents, bootstrap, models } = useAppStores()
const runView = computed(() => run.view.value)
const changeSummaries = computed(() => summarizeFileChanges(runView.value.fileChanges))
const selectedChangePath = ref<string | null>(null)
const selectedAgentId = ref<string | null>(null)
const activeInspectorTab = ref<InspectorTab>('context')
const inspectorOpen = ref(true)
const runConnectionStatus = computed(() => run.connectionStatus.value)
const runProjectionStatus = computed(() => run.projectionStatus.value)
const runErrorMessage = computed(() => run.error.value?.message ?? null)
const conversationStarted = computed(() => run.conversationStarted.value)
const agentStatus = computed(() => agents.status)
const agentStreamStatus = computed(() => agents.streamStatus)
const agentSnapshot = computed(() => agents.snapshot)
const agentError = computed(() => agents.error?.message ?? null)
const selectedAgent = computed(
  () => agentSnapshot.value?.agents.find((agent) => agent.agent_id === selectedAgentId.value) ?? null,
)
const workspaceName = computed(() => bootstrap.context.value?.workspace.name ?? null)
const modelCatalog = computed(() => models.catalog)
const modelStatus = computed(() => models.status)
const greeting = computed(() => t('workspace.greeting', { name: t('app.name') }))
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

const threadTitle = computed(() => selected.value?.title ?? t('transcript.newChatTitle'))

/**
 * The identity above the transcript.
 *
 * A run does not carry the agent it was delegated to, so the header falls back
 * to the fleet's own choice: the agent the user picked in the rail, or the one
 * the runtime reports as busy. Naming nobody is worse than naming the fleet's
 * guess — an empty header reads as "no one is answering".
 */
const threadAgent = computed(() => {
  const fleet = agentSnapshot.value?.agents ?? []
  return (
    fleet.find((agent) => agent.agent_id === selectedAgentId.value) ??
    fleet.find((agent) => agent.activity === 'running') ??
    fleet[0] ??
    null
  )
})

const threadAgentName = computed(() => threadAgent.value?.display_name ?? t('app.name'))

const threadAgentStatus = computed(() =>
  threadAgent.value ? t(agentActivityMessageKey(threadAgent.value)) : null,
)

const threadAgentOnline = computed(() => threadAgent.value?.availability === 'available')

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

function handleAgentSelect(agentId: string): void {
  selectedAgentId.value = agentId
  activeInspectorTab.value = 'context'
}

function handleInspectorTabChange(tab: InspectorTab): void {
  activeInspectorTab.value = tab
}

function handleOpenSettings(): void {
  void appRouter?.push({ name: 'settings' })
}
</script>

<template>
  <WorkspaceResponsive
    data-testid="workspace-view"
    :sessions-title="t('sessions.title')"
    :inspector-title="t('inspector.label')"
    :controls-label="t('inspector.label')"
    :inspector-open="inspectorOpen"
  >
    <template #sessions>
      <WorkspaceSidebar
        :product-name="t('app.name')"
        :workspace-name="workspaceName"
        :session-status="status"
        :sessions="items"
        :selected-session-id="selectedId"
        :next-cursor="nextCursor"
        :session-error="error"
        :agent-status="agentStatus"
        :agent-stream-status="agentStreamStatus"
        :agent-snapshot="agentSnapshot"
        :agent-error="agentError"
        :selected-agent-id="selectedAgentId"
        @select-session="sessions.select"
        @refresh-sessions="sessions.load"
        @load-more-sessions="sessions.loadMore"
        @new-session="sessions.select(null)"
        @select-agent="handleAgentSelect"
        @open-settings="handleOpenSettings"
      />
    </template>

    <ThreadBar
      :title="threadTitle"
      :agent-name="threadAgentName"
      :agent-status="threadAgentStatus"
      :agent-online="threadAgentOnline"
      :share-label="t('transcript.share')"
      :share-text="t('transcript.share')"
      :more-label="t('transcript.more')"
      :panel-label="t('transcript.togglePanel')"
      :panel-open="inspectorOpen"
      @toggle-panel="inspectorOpen = !inspectorOpen"
    />

    <RunPanel
      v-if="!selected"
      :view="runView"
      :empty-title="greeting"
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
      <InspectorDock
        :active-tab="activeInspectorTab"
        @update:active-tab="handleInspectorTabChange"
      >
        <template #context>
          <AgentDetails :agent="selectedAgent" />
        </template>
        <template #changes>
          <ChangeInspector
            :changes="changeSummaries"
            :selected-path="selectedChangePath"
            @select="selectedChangePath = $event"
          />
        </template>
      </InspectorDock>
    </template>
  </WorkspaceResponsive>
</template>
