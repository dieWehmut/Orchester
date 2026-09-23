<script setup lang="ts">
import { FolderOpen } from '@lucide/vue'

import { useI18n } from '../../i18n'

const { t } = useI18n()

withDefaults(
  defineProps<{
    workspaceName?: string | null
  }>(),
  {
    workspaceName: null,
  },
)
</script>

<template>
  <div class="composer-context" data-composer-context>
    <!-- The model is not here: which one the next run uses is a choice about the
         run, and the reference draws that control at the trailing edge of the
         field rather than as another fact about the workspace. -->
    <span class="composer-context__item" data-project-context>
      <FolderOpen :size="15" aria-hidden="true" />
      <span>{{ workspaceName || t('run.chooseProject') }}</span>
    </span>
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
