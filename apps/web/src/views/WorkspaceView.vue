<script setup lang="ts">
import { AgentDetails, agentActivityMessageKey } from '../features/agent-presence'
import ChangeInspector from '../components/changes/ChangeInspector.vue'
import { summarizeFileChanges } from '../components/changes/change-summary'
import InspectorDock from '../components/layout/InspectorDock.vue'
import type { InspectorTab } from '../components/layout/inspector-tabs'
import AppShell from '../components/layout/AppShell.vue'
import BottomPanel from '../components/layout/BottomPanel.vue'
import TabStrip from '../components/layout/TabStrip.vue'
import { reorderTab, type ShellTab } from '../components/layout/tab-strip'
import WorkspaceSidebar from '../components/layout/WorkspaceSidebar.vue'
import ThreadBar from '../components/layout/ThreadBar.vue'
import SessionTranscript from '../components/sessions/SessionTranscript.vue'
import RunPanel from '../components/run/RunPanel.vue'
import { useI18n } from '../i18n'
import { useAppStores } from '../stores/app'
import { useShortcut } from '../shortcuts'
import { computed, inject, ref } from 'vue'
import { routerKey } from 'vue-router'

const { t } = useI18n()
const appRouter = inject(routerKey, null)
const { sessions, run, agents, bootstrap, models } = useAppStores()
const runView = computed(() => run.view.value)
const runEvents = computed(() => run.events.value)
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
 * The strip's tabs, region B.
 *
 * The strip is unified, so a tab names what it opens rather than which pane
 * drew it: the run is always the first tab because the transcript cannot be
 * collapsed, and the inspector's tabs follow it as the surfaces the user has
 * opened. Closing a tab therefore selects a neighbour instead of emptying the
 * strip, which is the section 2 rule that the transcript stays.
 */
const shellTabs = computed<readonly ShellTab[]>(() => {
  const tabs: ShellTab[] = [{ id: 'run', kind: 'run', label: threadTitle.value }]
  tabs.push({ id: 'inspector', kind: 'agent', label: t('inspector.label') })
  if (changeSummaries.value.length > 0) {
    tabs.push({ id: 'changes', kind: 'diff', label: t('inspector.changes') })
  }
  return tabs
})
const activeTabId = ref('run')

function handleTabSelect(id: string): void {
  activeTabId.value = id
  if (id === 'changes') activeInspectorTab.value = 'changes'
  if (id === 'inspector') inspectorOpen.value = true
}

function handleTabClose(id: string): void {
  // The transcript is the product and cannot be closed, so the run tab reports
  // the close and stays; the others fold back into the inspector.
  if (id === 'run') return
  if (id === 'changes') activeInspectorTab.value = 'context'
  activeTabId.value = 'run'
}

function handleTabReorder(move: { from: string; to: string }): void {
  // The order lives in the strip's own list, so a reorder is applied by
  // rebuilding it - the run stays first, and the rest follow the drag.
  const order = reorderTab(shellTabs.value, move.from, move.to).map((tab) => tab.id)
  shellTabOrder.value = order
}

/** The user's order once they have dragged one, in preference to the default. */
const shellTabOrder = ref<readonly string[]>([])
const orderedShellTabs = computed<readonly ShellTab[]>(() => {
  if (shellTabOrder.value.length === 0) return shellTabs.value
  const byId = new Map(shellTabs.value.map((tab) => [tab.id, tab]))
  const ordered = shellTabOrder.value
    .map((id) => byId.get(id))
    .filter((tab): tab is ShellTab => tab !== undefined)
  for (const tab of shellTabs.value) if (!ordered.includes(tab)) ordered.push(tab)
  return ordered
})

/** The three surfaces region I holds, named through the locale. */
const bottomPanelTabs = computed(() => [
  { id: 'terminal', label: t('bottomPanel.terminal') },
  { id: 'output', label: t('bottomPanel.output') },
  { id: 'audit', label: t('bottomPanel.audit') },
])

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
const runLifecycle = computed(() => run.lifecycle.value)

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

/**
 * The shortcuts the workspace owns.
 *
 * Registered by the mounted view rather than by the shell, so the two panes
 * the shell cannot see 鈥?the inspector and the settings route 鈥?are only
 * bound while there is something to bind them to.
 */
useShortcut(
  {
    id: 'inspector.toggle',
    label: 'Toggle the inspector',
    group: 'Layout',
    keys: ['Mod', 'B'],
  },
  () => {
    inspectorOpen.value = !inspectorOpen.value
  },
)

useShortcut(
  {
    id: 'settings.open',
    label: 'Open settings',
    group: 'Layout',
    keys: ['Mod', ','],
  },
  handleOpenSettings,
)

</script>

<template>
  <TabStrip
    data-testid="workspace-tab-strip"
    :tabs="orderedShellTabs"
    :active-id="activeTabId"
    :label="t('tabStrip.label')"
    @select="handleTabSelect"
    @close="handleTabClose"
    @reorder="handleTabReorder"
  />
  <AppShell
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
      :events="runEvents"
      :empty-title="greeting"
      :connection-status="runConnectionStatus"
      :projection-status="runProjectionStatus"
      :error-message="runErrorMessage"
      :busy="runBusy"
      :lifecycle="runLifecycle"
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
  </AppShell>
  <BottomPanel
    :label="t('bottomPanel.label')"
    :tabs="bottomPanelTabs"
    data-testid="workspace-bottom-panel"
  >
    <template #terminal>
      <p class="workspace-view__panel-note">{{ t('bottomPanel.terminalEmpty') }}</p>
    </template>
    <template #output>
      <p class="workspace-view__panel-note">{{ t('bottomPanel.outputEmpty') }}</p>
    </template>
    <template #audit>
      <p class="workspace-view__panel-note">{{ t('bottomPanel.auditEmpty') }}</p>
    </template>
  </BottomPanel>
</template>

<style scoped>
.workspace-view__panel-note {
  margin: 0;
  padding: var(--space-3);
  color: var(--color-text-tertiary);
  font-size: var(--text-sm);
}
</style>
