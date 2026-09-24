<script setup lang="ts">
import { InlineAlert, StatusDot, useAppearance } from '@orchester/design'
import type { RunView } from '@orchester/ereignis'
import type { ModelCatalogDto, ModelSelectionRequestDto, UiEventEnvelope } from '@orchester/protokoll'
import { ArrowDown } from '@lucide/vue'
import { computed, nextTick, ref, watch } from 'vue'

import { useI18n } from '../../i18n'
import type { RunLifecycle } from '../../stores/run'
import ConnectionBanner, { type ConnectionBannerStatus } from './ConnectionBanner.vue'
import EmptyWorkspace from './EmptyWorkspace.vue'
import MessageRail from './MessageRail.vue'
import PlanStrip from './PlanStrip.vue'
import RunAnnouncer from './RunAnnouncer.vue'
import RunComposer from './RunComposer.vue'
import RunFooter from './RunFooter.vue'
import { runElapsed } from './run-duration'
import { runStatusMessageKey } from './run-status-label'
import RunTimeline from './RunTimeline.vue'
import { fadeDecision, readScrollState, stickDecision, unreadAfter, type ScrollState } from './scroll-state'
import { PetCompanion, petStateFor, usePetVisibility } from '../../features/pet'
import type { ModelCatalogStoreStatus } from '../../stores/model-catalog'

const props = withDefaults(
  defineProps<{
    view: RunView
    /** The journal the announcer reads; history is not announced. */
    events?: readonly UiEventEnvelope[]
    connectionStatus?: ConnectionBannerStatus
    projectionStatus?: 'idle' | 'ready' | 'gap' | 'error'
    busy?: boolean
    lifecycle?: RunLifecycle | null
    conversationStarted?: boolean
    errorMessage?: string | null
    /** Overridden by the workspace, which greets with the project name. */
    emptyTitle?: string
    emptyDescription?: string
    workspaceName?: string | null
    modelCatalog?: ModelCatalogDto | null
    modelStatus?: ModelCatalogStoreStatus
    /** The task whose run settings the composer reads and writes. */
    settingsKey?: string | null
    runStatus?: RunView['status']
    pendingApprovals?: number
    petLabel?: string
    petNotificationLabels?: Partial<Record<'running' | 'waiting' | 'review' | 'failed', string>>
  }>(),
  {
    events: () => [],
    connectionStatus: 'idle',
    projectionStatus: 'idle',
    busy: false,
    lifecycle: null,
    conversationStarted: false,
    errorMessage: null,
    workspaceName: null,
    modelCatalog: null,
    modelStatus: 'idle',
    settingsKey: null,
    runStatus: 'idle',
    pendingApprovals: 0,
    petLabel: '',
    petNotificationLabels: () => ({}),
  },
)

const emit = defineEmits<{
  submit: [prompt: string]
  cancel: []
  /** The model and effort the following runs should use. */
  'select-model': [selection: ModelSelectionRequestDto]
}>()

const { t } = useI18n()

/**
 * The strip the reference draws directly above the field.
 *
 * It is the run as it *is*, rather than the run as it was: the state, the title
 * the runtime gave the run, and a live clock. Only while it is in flight or
 * waiting on a human - once it has settled, the footer under the transcript is
 * the record, and a strip that stayed would be the same fact stated twice.
 *
 * The reference's strip carries delete, pause and expand controls. None of the
 * three is offered here: this runtime has no pause and no delete for a run in
 * flight, and the panel's own toggle is already in the thread bar above. The
 * stop stays where it is - on the field's own button, which changes shape while
 * a run is in flight - so the strip states and does not duplicate.
 */
const runState = computed(() => {
  const status = props.view.status
  const live = status === 'running' || status === 'awaiting_approval'
  if (!live) return null
  return {
    status,
    label: t(runStatusMessageKey(status)),
    title: props.view.title,
    elapsed: runElapsed(props.view),
    dot: status === 'running' ? ('running' as const) : ('waiting' as const),
  }
})

/**
 * The run's ledger states the run's own facts: its sequence and its tokens.
 *
 * The clock is not among them any more. The reference states a turn's elapsed
 * time at the turn - and each answer now carries its own, measured from the
 * entry before it - while a run still in flight is the strip's business. A third
 * copy under the transcript would be the same number in a place the reference
 * does not put it.
 */

/**
 * The draft the composer is editing.
 *
 * The panel owns it rather than the composer because the transcript's action row
 * writes into it: quoting an answer and putting a prompt back are both edits to
 * the next prompt, and the field is where they end.
 */
const composerDraft = ref('')
const composer = ref<InstanceType<typeof RunComposer> | null>(null)

/** Quote an answer into the field as markdown, which is how it will be read. */
function quoteAnswer(text: string): void {
  const quoted = text
    .split('\n')
    .map((line) => (line.trim().length > 0 ? `> ${line}` : '>'))
    .join('\n')
  composerDraft.value = `${quoted}\n\n`
  void nextTick(() => composer.value?.focus())
}

/** Put the reader's own prompt back, so it can be run again or edited first. */
function reusePrompt(text: string): void {
  composerDraft.value = text
  void nextTick(() => composer.value?.focus())
}

/**
 * Run the question an answer answered, again.
 *
 * The reference's own control regenerates the turn in place; this runtime starts
 * a run for the prompt instead, so the label says what happens. Nothing is put
 * in the field: the reader asked for another answer to a question they already
 * asked, not for the question back.
 */
function rerunPrompt(text: string): void {
  emit('submit', text)
}

/**
 * A run waiting on the user is the one case the plan strip has to escalate:
 * `awaiting_approval` means the next move is not the agent's to make.
 */
const planBlocked = computed(() => props.view.status === 'awaiting_approval')

const stream = ref<HTMLElement | null>(null)
const scrollState = ref<ScrollState>({ canScrollUp: false, canScrollDown: false, atBottom: true })

/** The fade under the header, drawn only when there is content above. */
const topFade = computed(() => fadeDecision(scrollState.value))

/**
 * Output that arrived while the reader was reading back. It is cleared the
 * moment they are at the bottom again, because then they have seen it.
 */
const unread = ref(0)

/** The scroller's own measurements, handed to the timeline that virtualises. */
const scrollTop = ref(0)
const viewportHeight = ref(0)

const unreadLabel = computed(() =>
  unread.value > 0
    ? t('transcript.unreadCount', { count: String(unread.value) })
    : t('transcript.unread'),
)

function measure(): void {
  const element = stream.value
  if (!element) return
  viewportHeight.value = element.clientHeight
  scrollTop.value = element.scrollTop
  scrollState.value = readScrollState({
    scrollTop: element.scrollTop,
    scrollHeight: element.scrollHeight,
    clientHeight: element.clientHeight,
  })
}

/**
 * The rail reports a timeline index; the transcript turns it into a scroll
 * position. The rows carry their index, so the jump is to the row the reader
 * was shown rather than to an estimate of where it might be.
 */
function scrollToTurn(index: number): void {
  const element = stream.value
  if (!element) return
  const row = element.querySelector<HTMLElement>(`[data-virtualized-turn="${index}"]`)
  if (!row) return
  row.scrollIntoView({ block: 'start' })
  measure()
}

function scrollToBottom(): void {
  const element = stream.value
  if (!element) return
  element.scrollTop = element.scrollHeight
  unread.value = 0
  measure()
}

/**
 * New output follows the run only while the reader is still at the bottom;
 * once they scroll back to read, the transcript holds still instead of
 * dragging the page away from them.
 */
watch(
  () => props.view.timeline.length,
  async () => {
    const position = scrollState.value.atBottom ? 'at-bottom' : 'reading-back'
    await nextTick()
    const decision = stickDecision(position, 'appended')
    unread.value = unreadAfter(unread.value, decision)
    if (decision === 'stick') scrollToBottom()
    else measure()
  },
)
const petVisibility = usePetVisibility()
const { prefersReducedMotion } = useAppearance()
const petState = computed(() =>
  petStateFor({
    runStatus: props.runStatus,
    busy: props.busy,
    pendingApprovals: props.pendingApprovals,
    errorMessage: props.errorMessage,
  }),
)
const petNotification = computed(() => {
  const kind = petState.value.notification
  return kind === null ? '' : (props.petNotificationLabels[kind] ?? '')
})

/**
 * Put the caret in the field, or empty it first.
 *
 * The chrome's Edit menu acts on the prompt, and the prompt's state lives
 * here: a clear that reached into the textarea from outside would be a second
 * writer racing the draft the panel already keeps.
 */
function focusPrompt(): void {
  void nextTick(() => composer.value?.focus())
}

function clearPrompt(): void {
  composerDraft.value = ''
  void nextTick(() => composer.value?.focus())
}

/**
 * Whether this is the page before there is anything on it.
 *
 * The reference greets a new chat with its greeting and its field as one group
 * in the middle, and docks the field to the bottom only once there is something
 * to scroll: a field pinned to the bottom under an empty page leaves the reader
 * looking at the space between them. So the two move together, and the run's
 * ledger under the transcript has nothing to report yet either.
 */
const hero = computed(() => !props.conversationStarted && props.view.timeline.length === 0)

defineExpose({ focusPrompt, clearPrompt })
</script>

<template>
  <section class="run-panel" data-run-panel :data-run-hero="hero ? 'true' : 'false'">
    <RunAnnouncer :events="props.events" />
    <ConnectionBanner :status="props.connectionStatus" />
    <InlineAlert v-if="props.errorMessage" tone="error" data-run-error>
      {{ props.errorMessage }}
    </InlineAlert>
    <div
      ref="stream"
      class="run-panel__stream"
      data-transcript-scroll
      :data-can-scroll-up="scrollState.canScrollUp"
      :data-can-scroll-down="scrollState.canScrollDown"
      @scroll.passive="measure"
    >
      <RunTimeline
        v-if="props.view.timeline.length > 0"
        :view="props.view"
        :scroll-top="scrollTop"
        :viewport-height="viewportHeight"
        @quote="quoteAnswer"
        @reuse="reusePrompt"
        @rerun="rerunPrompt"
      />
      <EmptyWorkspace
        v-else-if="!props.conversationStarted"
        :title="props.emptyTitle ?? t('run.newRun')"
        :description="props.emptyDescription ?? t('run.newRunDescription')"
        data-run-empty
      />
      <div v-else class="run-panel__awaiting" data-run-awaiting-events role="status" aria-live="polite">
        <span>{{ props.busy ? t('run.starting') : t('run.awaiting') }}</span>
      </div>
    </div>
    <div
      class="run-panel__top-fade"
      :data-transcript-fade="topFade"
      aria-hidden="true"
    />
    <button
      v-if="scrollState.canScrollDown"
      class="run-panel__to-bottom"
      type="button"
      data-scroll-to-bottom
      data-scroll-shape="jump"
      :data-scroll-unread="String(unread > 0)"
      :aria-label="unread > 0 ? unreadLabel : t('transcript.jumpToLatest')"
      @click="scrollToBottom"
    >
      <ArrowDown :size="16" aria-hidden="true" />
      <span
        v-if="unread > 0"
        class="run-panel__unread"
        data-scroll-unread-dot
        aria-hidden="true"
      >{{ unread }}</span>
    </button>
    <MessageRail :view="props.view" @select="scrollToTurn" />
    <RunFooter
      v-if="!hero"
      :view="props.view"
      :sequence-label="t('run.sequence')"
      :usage-label="t('run.usage')"
    />
    <PlanStrip
      :todos="props.view.todos"
      :validation="props.view.validation"
      :blocked="planBlocked"
    />
    <!--
      The companion reacts to a run - it has an animation for each state - so it
      belongs where a run is, not on the page before the first one. Drawn there it
      would also sit between the greeting and the field, which the reference
      draws as one group.
    -->
    <div
      v-if="petVisibility.visible.value && !hero"
      class="run-panel__companion"
      data-run-companion
    >
      <PetCompanion
        :animation="petState.animation"
        :label="petNotification || props.petLabel"
        :reduced-motion="prefersReducedMotion"
      />
    </div>
    <!--
      The run as it is, directly above the field where the reader is looking:
      the reference's own strip. It states; the field's button is what acts.
    -->
    <div
      v-if="runState !== null"
      class="run-panel__state"
      data-run-state
      :data-run-state-status="runState.status"
    >
      <StatusDot
        :status="runState.dot"
        :label="runState.label"
        :pulse="runState.status === 'running'"
      />
      <span class="run-panel__state-label" data-run-state-label>{{ runState.label }}</span>
      <span v-if="runState.title" class="run-panel__state-title" data-run-state-title>
        {{ runState.title }}
      </span>
      <span v-if="runState.elapsed" class="run-panel__state-elapsed" data-run-state-elapsed>
        {{ t('transcript.took', { duration: runState.elapsed }) }}
      </span>
    </div>
    <RunComposer
      ref="composer"
      :model-value="composerDraft"
      :busy="props.busy"
      :lifecycle="props.lifecycle"
      :workspace-name="props.workspaceName"
      :model-catalog="props.modelCatalog"
      :model-status="props.modelStatus"
      :settings-key="props.settingsKey"
      @update:model-value="composerDraft = $event"
      @submit="emit('submit', $event)"
      @cancel="emit('cancel')"
      @select-model="emit('select-model', $event)"
    />
  </section>
</template>

<style scoped>
.run-panel__state {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-1) var(--space-3);
  color: var(--color-text-secondary);
  font-size: var(--text-xs);
}

.run-panel__state-label {
  color: var(--color-text-primary);
  font-weight: var(--weight-medium);
}

.run-panel__state-title {
  overflow: hidden;
  min-inline-size: 0;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* The clock sits at the far end, as the reference seats it. */
.run-panel__state-elapsed {
  margin-inline-start: auto;
  white-space: nowrap;
}

.run-panel {
  position: relative;
  display: flex;
  min-block-size: 100%;
  flex-direction: column;
  background: var(--color-bg-base);
}

.run-panel__stream {
  min-block-size: 0;
  flex: 1;
  overflow: auto;
}

/* The greeting and the field are one group in the middle of an empty page, as
   the reference draws them; there is nothing to scroll and nothing to dock
   under. */
.run-panel[data-run-hero='true'] {
  justify-content: center;
  gap: var(--space-4);
}

.run-panel[data-run-hero='true'] .run-panel__stream {
  flex: 0 1 auto;
  overflow: visible;
}

.run-panel__top-fade {
  position: absolute;
  inset-block-start: 0;
  inset-inline: 0;
  block-size: var(--space-6);
  background: linear-gradient(to bottom, var(--color-bg-base), transparent);
  pointer-events: none;
}

.run-panel__top-fade[data-transcript-fade='hidden'] {
  opacity: 0;
}

.run-panel__unread {
  position: absolute;
  inset-block-start: calc(-1 * var(--space-1));
  inset-inline-end: calc(-1 * var(--space-1));
  min-inline-size: var(--space-4);
  padding-inline: var(--space-1);
  border-radius: var(--radius-full);
  background: var(--color-accent);
  color: var(--color-accent-contrast);
  font-size: var(--text-xs);
  font-variant-numeric: tabular-nums;
  line-height: var(--font-text-xs-line-height);
  text-align: center;
}

.run-panel__to-bottom {
  position: relative;
  align-self: center;
  display: inline-grid;
  place-items: center;
  inline-size: var(--hit-target-min, 32px);
  min-block-size: var(--hit-target-min, 32px);
  min-inline-size: var(--hit-target-min, 32px);
  margin-block-start: calc(-1 * var(--space-6));
  padding: 0;
  border: 1px solid var(--color-border-base);
  border-radius: var(--radius-full);
  background: var(--color-bg-surface);
  box-shadow: var(--shadow-200);
  color: var(--color-text-secondary);
  cursor: pointer;
}

.run-panel :deep(.run-composer) {
  inline-size: calc(100% - var(--space-8));
  margin-block: var(--space-3) var(--space-4);
}

.run-panel__companion {
  display: grid;
  justify-items: center;
  padding-block-start: var(--space-2);
}

.run-panel__awaiting {
  display: grid;
  min-block-size: 100%;
  place-items: center;
  color: var(--color-text-tertiary);
  font-size: var(--text-sm);
}

@media (max-width: 640px) {
  .run-panel :deep(.run-composer) {
    inline-size: calc(100% - var(--space-4));
    margin-block: var(--space-2);
  }
}
</style>
