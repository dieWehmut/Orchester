<script setup lang="ts">
import { EmptyState, InlineAlert } from '@orchester/design'
import type { RunView } from '@orchester/ereignis'

import ConnectionBanner, { type ConnectionBannerStatus } from './ConnectionBanner.vue'
import RunComposer from './RunComposer.vue'
import RunFooter from './RunFooter.vue'
import RunTimeline from './RunTimeline.vue'

const props = withDefaults(
  defineProps<{
    view: RunView
    connectionStatus?: ConnectionBannerStatus
    projectionStatus?: 'idle' | 'ready' | 'gap' | 'error'
    busy?: boolean
    errorMessage?: string | null
    emptyTitle?: string
    emptyDescription?: string
  }>(),
  {
    connectionStatus: 'idle',
    projectionStatus: 'idle',
    busy: false,
    errorMessage: null,
    emptyTitle: 'New run',
    emptyDescription: 'Start a run to see events here.',
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
      <EmptyState
        v-else
        :title="props.emptyTitle"
        :description="props.emptyDescription"
        data-run-empty
      />
    </div>
    <RunFooter :view="props.view" />
    <RunComposer :busy="props.busy" @submit="emit('submit', $event)" @cancel="emit('cancel')" />
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
</style>
