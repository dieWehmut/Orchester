<script setup lang="ts">
import { AgentDetails, agentActivityMessageKey } from '../features/agent-presence'
import ChangeInspector from '../components/changes/ChangeInspector.vue'
import ReviewPanel from '../components/changes/ReviewPanel.vue'
import { turnIndexByPath, withTurns } from '../components/changes/review-filters'
import { summarizeFileChanges } from '../components/changes/change-summary'
import InspectorDock from '../components/layout/InspectorDock.vue'
import type { InspectorTab } from '../components/layout/inspector-tabs'
import AppShell from '../components/layout/AppShell.vue'
import BottomPanel from '../components/layout/BottomPanel.vue'
import TabStrip from '../components/layout/TabStrip.vue'
import { reorderTab, type ShellTab } from '../components/layout/tab-strip'
import {
  readTerminalPlacement,
  type TerminalPlacement,
} from '../components/layout/terminal-placement'
import {
  readTabStripState,
  writeTabStripState,
} from '../components/layout/tab-strip-persistence'
import WorkspaceSidebar from '../components/layout/WorkspaceSidebar.vue'
import ThreadBar from '../components/layout/ThreadBar.vue'
import SessionTranscript from '../components/sessions/SessionTranscript.vue'
import RunPanel from '../components/run/RunPanel.vue'
import { useI18n } from '../i18n'
import { useAppStores } from '../stores/app'
import { useShortcut } from '../shortcuts'
import { computed, inject, onMounted, ref, watch } from 'vue'
import { routerKey } from 'vue-router'

const { t } = useI18n()
const appRouter = inject(routerKey, null)
const { sessions, run, agents, bootstrap, models, review } = useAppStores()
const runView = computed(() => run.view.value)
const runEvents = computed(() => run.events.value)
const changeSummaries = computed(() => summarizeFileChanges(runView.value.fileChanges))
/**
 * The Review tab's change set, section 4.7.
 *
 * Two authorities answer one question: the runtime knows where a change sits
 * in the working copy, and the run knows which turn produced it. The join is
 * by path, and a path the run never reported belongs to no turn rather than
 * to a guessed one.
 */
const reviewChanges = computed(() =>
  withTurns(
    review.changes,
    turnIndexByPath(runView.value.turns, runView.value.fileChanges),
  ),
)
const reviewLastTurn = computed(() => runView.value.turns.length || null)
const selectedChangePath = ref<string | null>(null)
const selectedAgentId = ref<string | null>(null)
const activeInspectorTab = ref<InspectorTab>('context')
/**
 * Where the terminal lives, section 4.7.
 *
 * The preference is read once on mount and written when the reader changes the
 * setting, so the two surfaces below can be composed from it rather than each
 * deciding for itself where the terminal goes.
 */
const terminalPlacement = ref<TerminalPlacement>(readTerminalPlacement())
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
    tabs.push({ id: 'changes', kind: 'diff', label: t('inspector.review') })
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

/**
 * The shell's remembered tab state.
 *
 * Read once on mount rather than at setup so the first paint is the strip the
 * runtime can always serve - the run alone - and the stored order lands on top
 * of it. Written on every change so a window closed mid-session reopens on the
 * surface the user was looking at.
 */
onMounted(() => {
  const stored = readTabStripState()
  if (stored.order.length > 0) shellTabOrder.value = stored.order
  const ids = orderedShellTabs.value.map((tab) => tab.id)
  if (stored.activeId !== null && ids.includes(stored.activeId)) {
    activeTabId.value = stored.activeId
    if (stored.activeId === 'changes') activeInspectorTab.value = 'changes'
  }
})

function persistTabs(): void {
  writeTabStripState({
    order: orderedShellTabs.value.map((tab) => tab.id),
    activeId: activeTabId.value,
  })
}

watch([orderedShellTabs, activeTabId], persistTabs)


/**
 * The surfaces region I holds, named through the locale, section 2.
 *
 * The terminal is only one of them while the preference puts it there: a
 * preference that moves the terminal to the inspector has to take it off the
 * panel, or the reader gets two terminals and a panel whose first tab opens
 * nothing they asked for.
 */
const bottomPanelTabs = computed(() =>
  [
    { id: 'terminal', label: t('bottomPanel.terminal'), placement: 'bottom' as const },
    { id: 'output', label: t('bottomPanel.output'), placement: null },
    { id: 'audit', label: t('bottomPanel.audit'), placement: null },
  ]
    .filter((tab) => tab.placement === null || tab.placement === terminalPlacement.value)
    .map(({ id, label }) => ({ id, label })),
)

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

/**
 * The task the composer's settings belong to. Section 4.6 keeps the model, the
 * effort and the approval preset on the task rather than on the user, so the
 * key is the open task; an unstarted one has no identity yet and gets none.
 */
const runSettingsKey = computed(() => selectedId.value)

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
      :settings-key="runSettingsKey"
      @submit="handleRunSubmit"
      @cancel="handleRunCancel"
    />
    <SessionTranscript v-else :status="detailStatus" :session="selected" :error="detailError" />

    <template #inspector>
      <InspectorDock
        :active-tab="activeInspectorTab"
        :terminal="terminalPlacement === 'inspector'"
        @update:active-tab="handleInspectorTabChange"
      >
        <template #context>
          <AgentDetails :agent="selectedAgent" />
        </template>
        <template #changes>
          <ReviewPanel
            :changes="reviewChanges"
            :branch-changes="review.branchChanges"
            :last-turn="reviewLastTurn"
          />
          <ChangeInspector
            :changes="changeSummaries"
            :selected-path="selectedChangePath"
            @select="selectedChangePath = $event"
          />
        </template>
        <template #terminal>
          <p class="workspace-view__panel-note">{{ t('bottomPanel.terminalEmpty') }}</p>
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
