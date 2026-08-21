<script setup lang="ts">
import { InlineAlert } from '@orchester/design'
import type { RunView } from '@orchester/ereignis'
import type { ModelCatalogDto } from '@orchester/protokoll'

import ConnectionBanner, { type ConnectionBannerStatus } from './ConnectionBanner.vue'
import EmptyWorkspace from './EmptyWorkspace.vue'
import RunComposer from './RunComposer.vue'
import RunFooter from './RunFooter.vue'
import RunTimeline from './RunTimeline.vue'
import type { ModelCatalogStoreStatus } from '../../stores/model-catalog'

const props = withDefaults(
  defineProps<{
    view: RunView
    connectionStatus?: ConnectionBannerStatus
    projectionStatus?: 'idle' | 'ready' | 'gap' | 'error'
    busy?: boolean
    conversationStarted?: boolean
    errorMessage?: string | null
    emptyTitle?: string
    emptyDescription?: string
    workspaceName?: string | null
    modelCatalog?: ModelCatalogDto | null
    modelStatus?: ModelCatalogStoreStatus
  }>(),
  {
    connectionStatus: 'idle',
    projectionStatus: 'idle',
    busy: false,
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
</script>

<template>
  <section class="run-panel" data-run-panel>
    <ConnectionBanner :status="props.connectionStatus" />
    <InlineAlert v-if="props.errorMessage" tone="error" data-run-error>
      {{ props.errorMessage }}
    </InlineAlert>
    <div class="run-panel__stream">
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
    <RunFooter :view="props.view" />
    <RunComposer
      :busy="props.busy"
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

.run-panel__awaiting {
  display: grid;
  min-block-size: 100%;
  place-items: center;
  color: var(--color-text-tertiary);
  font-size: var(--text-sm);
}
</style>
