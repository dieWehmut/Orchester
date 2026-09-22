<script setup lang="ts">
import { InlineAlert, useAppearance } from '@orchester/design'
import type { RunView } from '@orchester/ereignis'
import type { ModelCatalogDto, UiEventEnvelope } from '@orchester/protokoll'
import { computed, nextTick, onUnmounted, ref, watch } from 'vue'

import { useI18n } from '../../i18n'
import type { RunLifecycle } from '../../stores/run'
import ConnectionBanner, { type ConnectionBannerStatus } from './ConnectionBanner.vue'
import EmptyWorkspace from './EmptyWorkspace.vue'
import MessageRail from './MessageRail.vue'
import PlanStrip from './PlanStrip.vue'
import RunAnnouncer from './RunAnnouncer.vue'
import RunComposer from './RunComposer.vue'
import RunFooter from './RunFooter.vue'
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
}>()

const { t } = useI18n()

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
 * The travel the companion's band offers it.
 *
 * The sprite travels along the band the panel draws above the composer, so the
 * band's own width is the distance it may cover. It is measured rather than
 * assumed because the band is as wide as the transcript column, which the
 * window decides, and because the sprite moves by transform - the band does not
 * grow to hold it, so the span has to leave the sprite its own width behind.
 */
const companionBand = ref<HTMLElement | null>(null)
const companionSpan = ref(0)
let companionObserver: ResizeObserver | null = null

function measureCompanionSpan(): void {
  const band = companionBand.value
  if (!band) {
    companionSpan.value = 0
    return
  }
  const styles = typeof getComputedStyle === 'function' ? getComputedStyle(band) : null
  const inlinePad =
    (parseFloat(styles?.paddingInlineStart ?? '') || 0) +
    (parseFloat(styles?.paddingInlineEnd ?? '') || 0)
  const width = Math.max((band.clientWidth ?? 0) - inlinePad, 0)
  const sprite = band.querySelector<HTMLElement>('[data-pet-companion]')
  const spriteWidth = sprite?.offsetWidth ?? 0
  companionSpan.value = Math.max(width - spriteWidth, 0)
}

watch(companionBand, (band) => {
  companionObserver?.disconnect()
  companionObserver = null
  measureCompanionSpan()
  if (band && typeof ResizeObserver === 'function') {
    companionObserver = new ResizeObserver(measureCompanionSpan)
    companionObserver.observe(band)
  }
})

onUnmounted(() => {
  companionObserver?.disconnect()
  companionObserver = null
})
</script>

<template>
  <section class="run-panel" data-run-panel>
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
      :data-scroll-unread="String(unread > 0)"
      @click="scrollToBottom"
    >
      <span
        v-if="unread > 0"
        class="run-panel__unread"
        data-scroll-unread-dot
        role="status"
        aria-live="polite"
      >{{ unreadLabel }}</span>
      <span v-else>{{ t('transcript.jumpToLatest') }}</span>
    </button>
    <MessageRail :view="props.view" @select="scrollToTurn" />
    <RunFooter :view="props.view" />
    <PlanStrip
      :todos="props.view.todos"
      :validation="props.view.validation"
      :blocked="planBlocked"
    />
    <div
      v-if="petVisibility.visible.value"
      ref="companionBand"
      class="run-panel__companion"
      data-run-companion
    >
      <PetCompanion
        :animation="petState.animation"
        :label="petNotification || props.petLabel"
        :reduced-motion="prefersReducedMotion"
        roam
        :roam-span="companionSpan"
      />
    </div>
    <RunComposer
      :busy="props.busy"
      :lifecycle="props.lifecycle"
      :workspace-name="props.workspaceName"
      :model-catalog="props.modelCatalog"
      :model-status="props.modelStatus"
      :settings-key="props.settingsKey"
      @submit="emit('submit', $event)"
      @cancel="emit('cancel')"
    />
  </section>
</template>

<style scoped>
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
  font-variant-numeric: tabular-nums;
}

.run-panel__to-bottom {
  align-self: center;
  min-block-size: var(--hit-target-min, 32px);
  margin-block-start: calc(-1 * var(--space-6));
  padding: var(--space-1) var(--space-3);
  border: 1px solid var(--color-border-base);
  border-radius: 999px;
  background: var(--color-bg-surface);
  box-shadow: var(--shadow-200);
  color: var(--color-text-secondary);
  cursor: pointer;
  font-size: var(--text-xs);
}

.run-panel :deep(.run-composer) {
  inline-size: calc(100% - var(--space-8));
  margin-block: var(--space-3) var(--space-4);
}

.run-panel__companion {
  /*
   * The band is the companion's stage: the sprite paces its width rather than
   * sitting centred in it, so the sprite starts at the band's leading edge and
   * travels from there. It stays in flow rather than being positioned: the
   * band's height is then the sprite's own height, which is what keeps the
   * stage from inventing a height the companion has to fit into.
   */
  display: flex;
  justify-content: flex-start;
  padding-inline: var(--space-4);
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
