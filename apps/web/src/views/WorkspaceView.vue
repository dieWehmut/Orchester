<script setup lang="ts">
import { AppButton, AppDialog } from '@orchester/design'
import { AgentDetails, agentActivityMessageKey } from '../features/agent-presence'
import ChangeInspector from '../components/changes/ChangeInspector.vue'
import ApprovalsQueue from '../components/changes/ApprovalsQueue.vue'
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
import { usePetVisibility } from '../features/pet'
import { useAppStores } from '../stores/app'
import { useShortcut } from '../shortcuts'
import {
  registerShellAction,
  useShellAction,
  useShellActionState,
} from '../components/layout/shell-actions'
import { DESKTOP_WINDOW_KEY } from '../platform/desktop-window'
import { computed, inject, onMounted, onUnmounted, ref, watch } from 'vue'
import { routerKey } from 'vue-router'

const { t } = useI18n()
const appRouter = inject(routerKey, null)
const { sessions, run, agents, bootstrap, models, review, approvals } = useAppStores()
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
const runStatus = computed(() => runView.value.status)
const pendingApprovals = computed(
  () => runView.value.approvals.filter((approval) => approval.state === 'pending').length,
)
const petNotificationLabels = computed<
  Partial<Record<'running' | 'waiting' | 'review' | 'failed', string>>
>(() => ({
  running: t('pet.notification.running'),
  waiting: t('pet.notification.waiting'),
  review: t('pet.notification.review'),
  failed: t('pet.notification.failed'),
}))
/**
 * The companion's visibility, as the account menu offers it.
 *
 * The menu names the action rather than the state - the reference's row reads
 * "show pet" whether or not the pet is drawn - so the label swaps between the
 * two words the catalogue already carries for the toggle.
 */
const petVisibility = usePetVisibility()
const companionLabel = computed(() =>
  petVisibility.visible.value ? t('pet.hide') : t('pet.show'),
)
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
  if (inspectorTabOpen.value) {
    tabs.push({ id: 'inspector', kind: 'agent', label: t('inspector.label') })
  }
  if (inspectorTabOpen.value && changeSummaries.value.length > 0) {
    tabs.push({ id: 'changes', kind: 'diff', label: t('inspector.review') })
  }
  return tabs
})
const activeTabId = ref('run')

/**
 * The transcript's field, for the chrome's prompt actions.
 *
 * The chrome's Edit menu offers "focus the prompt" and "clear the input". Both
 * are intents on a field this view owns, so the view registers them and the row
 * draws them enabled only while the workspace - and therefore the composer - is
 * the route underneath.
 */
const runPanel = ref<InstanceType<typeof RunPanel> | null>(null)

useShellAction('prompt.focus', () => runPanel.value?.focusPrompt())
useShellAction('prompt.clear', () => runPanel.value?.clearPrompt())
useShellAction('companion.toggle', () => petVisibility.toggle())

/**
 * Whether the inspector is showing, as the chrome's toggle reports it.
 *
 * Registered beside the action so the row's control says what pressing it does
 * rather than what a remembered boolean last said.
 */
useShellActionState('inspector.toggle', () => inspectorOpen.value)

/**
 * A tab the reader can close, which is never the transcript.
 *
 * The transcript is the product and the one tab that cannot be closed, so the
 * chrome's File menu offers its row only while another tab is in front. The
 * registration follows the strip instead of the menu guessing at it.
 */
let unregisterCloseTab: (() => void) | null = null

watch(activeTabId, (id) => {
  unregisterCloseTab?.()
  unregisterCloseTab = id === 'run' ? null : registerShellAction('close-tab', () => handleTabClose(id))
})

onUnmounted(() => {
  unregisterCloseTab?.()
  unregisterCloseTab = null
})

/**
 * Whether the inspector's own tab is open in the strip.
 *
 * The transcript is the product and cannot be closed, so it is the tab left
 * when the others are gone. Closing the inspector's tab therefore folds the
 * inspector away rather than emptying the strip, and the pane comes back - with
 * its tab - when the reader asks for it again.
 */
const inspectorTabOpen = ref(true)

function handleTabSelect(id: string): void {
  activeTabId.value = id
  if (id === 'changes') activeInspectorTab.value = 'changes'
  if (id === 'inspector') inspectorOpen.value = true
}

function handleTabClose(id: string): void {
  if (id === 'run') return
  // Closing the strip's inspector tab closes the surface it names. The tab
  // that remains is the transcript, which is also what makes the next close
  // chord a window close.
  inspectorTabOpen.value = false
  inspectorOpen.value = false
  if (id === 'changes') activeInspectorTab.value = 'context'
  activeTabId.value = 'run'
}

// Opening the inspector from anywhere else brings its tab back, so the strip
// cannot disagree with the pane it names.
watch([inspectorOpen, activeInspectorTab], ([open]) => {
  if (open) inspectorTabOpen.value = true
})

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

/**
 * The header bell opens the approvals the count is reporting.
 *
 * The bell is only drawn when something is waiting, so following it has to
 * land on the queue those items are in: opening the inspector on some other
 * tab would leave the reader looking at a count with nothing behind it.
 */
function handleOpenAttention(): void {
  inspectorOpen.value = true
  activeInspectorTab.value = 'approvals'
}

/**
 * A decision the reader took in the queue.
 *
 * The row version is what makes a stale entry detectable, so it is sent back
 * with the decision rather than resolved here: the runtime owns the row and
 * only it can say whether the version the reader saw is still current.
 */
function handleApprovalDecision(decision: {
  approvalId: string
  rowVersion: number
  decision: 'approved' | 'denied'
}): void {
  const activeRunId = run.runId.value
  if (activeRunId === null) return
  void approvals.decide(activeRunId, decision)
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
    // Section 2.1 gives `Mod+B` to the rail's collapse, so the inspector's
    // toggle keeps a chord one modifier over. Two panes may not share a
    // gesture: the editor would list one binding, and only one would answer.
    id: 'inspector.toggle',
    labelKey: 'shortcuts.labels.inspectorToggle',
    groupKey: 'shortcuts.groups.layout',
    keys: ['Mod', 'Alt', 'B'],
  },
  () => {
    inspectorOpen.value = !inspectorOpen.value
  },
)

useShortcut(
  {
    id: 'settings.open',
    labelKey: 'shortcuts.labels.settingsOpen',
    groupKey: 'shortcuts.groups.layout',
    keys: ['Mod', ','],
  },
  handleOpenSettings,
)

// The companion's chord belongs to the view that owns the companion, so the
// editor cannot list a binding nothing would answer while this route is open.
useShortcut(
  {
    id: 'companion.toggle',
    labelKey: 'shortcuts.labels.companionToggle',
    groupKey: 'shortcuts.groups.layout',
    keys: ['Mod', 'Alt', 'P'],
  },
  () => petVisibility.toggle(),
)

/**
 * The desktop window, when there is one.
 *
 * The shell provides it, so the browser runtime - and every page that is not
 * the desktop app - injects nothing and keeps the chord for the browser's own
 * tab handling instead.
 */
const desktopWindowController = inject(DESKTOP_WINDOW_KEY, null)
const windowCloseConfirmOpen = ref(false)

/**
 * The close chord, section 8 of the design spec.
 *
 * Mod+W closes the active tab while there is one to close. Once the transcript
 * is the only tab left, the chord means the window itself, and a run that is
 * still working is confirmed first - closing the window must not silently
 * discard work the reader cannot see again.
 */
function handleWindowCloseChord(): void {
  if (activeTabId.value !== 'run') {
    handleTabClose(activeTabId.value)
    return
  }
  if (runBusy.value) {
    windowCloseConfirmOpen.value = true
    return
  }
  void desktopWindowController?.close()
}

async function confirmWindowClose(): Promise<void> {
  windowCloseConfirmOpen.value = false
  await desktopWindowController?.close()
}

function cancelWindowClose(): void {
  windowCloseConfirmOpen.value = false
}

// The chord is bound only where the app can answer both halves of it. In a
// browser the second half - hiding the window - is the browser's own tab, and
// claiming the chord there would take that away to do nothing with it.
if (desktopWindowController?.enabled) {
  useShortcut(
    {
      id: 'window.close',
      labelKey: 'shortcuts.labels.windowClose',
      groupKey: 'shortcuts.groups.layout',
      keys: ['Mod', 'W'],
    },
    handleWindowCloseChord,
  )
}

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
        :companion-label="companionLabel"
        :search-label="t('sessions.searchPlaceholder')"
        :attention-label="t('inspector.approvals')"
        :attention-count="pendingApprovals"
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
        @toggle-companion="petVisibility.toggle()"
        @open-attention="handleOpenAttention"
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
      :run-status="runStatus"
      :pending-approvals="pendingApprovals"
      :pet-label="t('pet.label')"
      :pet-notification-labels="petNotificationLabels"
      @submit="handleRunSubmit"
      @cancel="handleRunCancel"
      @select-model="models.select($event)"
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
        <template #approvals>
          <ApprovalsQueue
            :approvals="runView.approvals"
            :superseded="approvals.superseded"
            @decide="handleApprovalDecision"
          />
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
  <AppDialog
    :open="windowCloseConfirmOpen"
    data-window-close-confirm
    :title="t('window.closeTitle')"
    :description="t('window.closeDescription')"
    :close-label="t('window.closeCancel')"
    @update:open="(open) => { if (!open) cancelWindowClose() }"
  >
    <template #footer>
      <AppButton
        variant="ghost"
        size="sm"
        data-window-close-confirm="cancel"
        @click="cancelWindowClose"
      >
        {{ t('window.closeCancel') }}
      </AppButton>
      <AppButton
        variant="danger"
        size="sm"
        data-window-close-confirm="accept"
        @click="confirmWindowClose"
      >
        {{ t('window.closeAccept') }}
      </AppButton>
    </template>
  </AppDialog>
</template>

<style scoped>
.workspace-view__panel-note {
  margin: 0;
  padding: var(--space-3);
  color: var(--color-text-tertiary);
  font-size: var(--text-sm);
}
</style>
