<script setup lang="ts">
import { FolderGit2 } from '@lucide/vue'
import { computed } from 'vue'

const props = defineProps<{
  workspaceName: string | null
  fallbackLabel: string
}>()

const label = computed(() => props.workspaceName ?? props.fallbackLabel)
</script>

<template>
  <ul class="project-list" data-project-list>
    <li class="project-list__item" data-project-row>
      <FolderGit2 :size="15" aria-hidden="true" />
      <span data-project-name>{{ label }}</span>
    </li>
  </ul>
</template>

<style scoped>
.project-list {
  display: grid;
  gap: var(--space-1);
  margin: 0;
  padding: 0;
  list-style: none;
}

.project-list__item {
  display: flex;
  min-block-size: var(--control-height-sm);
  align-items: center;
  gap: var(--space-2);
  padding-inline: var(--space-2);
  border-radius: var(--radius-sm);
  color: var(--color-text-secondary);
  font-size: var(--text-sm);
}

.project-list__item span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
