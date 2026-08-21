<script setup lang="ts">
import { FolderOpen, ShieldCheck } from '@lucide/vue'
import type { ModelCatalogDto } from '@orchester/protokoll'

import type { ModelCatalogStoreStatus } from '../../stores/model-catalog'
import ModelContextControl from './ModelContextControl.vue'

withDefaults(
  defineProps<{
    workspaceName?: string | null
    modelCatalog?: ModelCatalogDto | null
    modelStatus?: ModelCatalogStoreStatus
    approvalLabel?: string
  }>(),
  {
    workspaceName: null,
    modelCatalog: null,
    modelStatus: 'idle',
    approvalLabel: 'Ask for approval',
  },
)
</script>

<template>
  <div class="composer-context" data-composer-context>
    <span class="composer-context__item" data-project-context>
      <FolderOpen :size="15" aria-hidden="true" />
      <span>{{ workspaceName || 'Choose project' }}</span>
    </span>
    <span class="composer-context__item" data-approval-context>
      <ShieldCheck :size="15" aria-hidden="true" />
      <span>{{ approvalLabel }}</span>
    </span>
    <ModelContextControl :catalog="modelCatalog" :status="modelStatus" />
  </div>
</template>

<style scoped>
.composer-context {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-3);
  padding: 0 var(--space-4) var(--space-3);
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
}

.composer-context__item {
  display: inline-flex;
  min-inline-size: 0;
  align-items: center;
  gap: var(--space-2);
}

.composer-context__item span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

@media (max-width: 640px) {
  .composer-context {
    gap: var(--space-2);
    padding-inline: var(--space-3);
  }
}
</style>
