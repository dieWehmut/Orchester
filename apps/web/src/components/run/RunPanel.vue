<script setup lang="ts">
import { InlineAlert } from '@orchester/design'
import type { RunView } from '@orchester/ereignis'
import type { ModelCatalogDto, UiEventEnvelope } from '@orchester/protokoll'
import { computed, nextTick, ref, watch } from 'vue'

import type { RunLifecycle } from '../../stores/run'
import ConnectionBanner, { type ConnectionBannerStatus } from './ConnectionBanner.vue'
import EmptyWorkspace from './EmptyWorkspace.vue'
import PlanStrip from './PlanStrip.vue'
import RunAnnouncer from './RunAnnouncer.vue'
import RunComposer from './RunComposer.vue'
import RunFooter from './RunFooter.vue'
import RunTimeline from './RunTimeline.vue'
import { readScrollState, stickDecision, type ScrollState } from './scroll-state'
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
    emptyTitle?: string
    emptyDescription?: string
    workspaceName?: string | null
    modelCatalog?: ModelCatalogDto | null
    modelStatus?: ModelCatalogStoreStatus
  }>(),
  {
    events: () => [],
    connectionStatus: 'idle',
    projectionStatus: 'idle',
    busy: false,
    lifecycle: null,
    conversationStarted: false,
    errorMessage: null,
    emptyTitle: 'New run',
    emptyDescription: 'Start a run to see events here.',
    workspaceName: null,
    modelCatalog: null,
    modelStatus: 'idle',
  },
)

const emit = defineEmits<{
  submit: [prompt: string]
  cancel: []
}>()

/**
 * A run waiting on the user is the one case the plan strip has to escalate:
 * `awaiting_approval` means the next move is not the agent's to make.
 */
const planBlocked = computed(() => props.view.status === 'awaiting_approval')

const stream = ref<HTMLElement | null>(null)
const scrollState = ref<ScrollState>({ canScrollUp: false, canScrollDown: false, atBottom: true })

function measure(): void {
  const element = stream.value
  if (!element) return
  scrollState.value = readScrollState({
    scrollTop: element.scrollTop,
    scrollHeight: element.scrollHeight,
    clientHeight: element.clientHeight,
  })
}

function scrollToBottom(): void {
  const element = stream.value
  if (!element) return
  element.scrollTop = element.scrollHeight
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
    if (stickDecision(position, 'appended') === 'stick') scrollToBottom()
    else measure()
  },
)
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
      <RunTimeline v-if="props.view.timeline.length > 0" :view="props.view" />
      <EmptyWorkspace
        v-else-if="!props.conversationStarted"
        :title="props.emptyTitle"
        :description="props.emptyDescription"
        data-run-empty
      />
      <div v-else class="run-panel__awaiting" data-run-awaiting-events role="status" aria-live="polite">
        <span>{{ props.busy ? 'Starting run…' : 'Waiting for run events…' }}</span>
      </div>
    </div>
    <button
      v-if="scrollState.canScrollDown"
      class="run-panel__to-bottom"
      type="button"
      data-scroll-to-bottom
      @click="scrollToBottom"
    >
      Jump to the latest
    </button>
    <RunFooter :view="props.view" />
    <PlanStrip
      :todos="props.view.todos"
      :validation="props.view.validation"
      :blocked="planBlocked"
    />
    <RunComposer
      :busy="props.busy"
      :lifecycle="props.lifecycle"
      :workspace-name="props.workspaceName"
      :model-catalog="props.modelCatalog"
      :model-status="props.modelStatus"
      @submit="emit('submit', $event)"
      @cancel="emit('cancel')"
    />
  </section>
</template>

<style scoped>
.run-panel {
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
