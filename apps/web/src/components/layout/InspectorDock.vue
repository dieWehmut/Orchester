<script setup lang="ts">
import { AppTabs, EmptyState, type AppTabOption } from '@orchester/design'
import { computed, ref } from 'vue'

import { useI18n } from '../../i18n'

type InspectorTab = 'context' | 'approvals' | 'changes'

const { t } = useI18n()
const activeTab = ref<InspectorTab>('context')
const tabs = computed<AppTabOption[]>(() => [
  { id: 'context', label: t('inspector.context') },
  { id: 'approvals', label: t('inspector.approvals') },
  { id: 'changes', label: t('inspector.changes') },
])
const panel = computed(() => ({
  context: {
    title: t('inspector.contextTitle'),
    description: t('inspector.contextDescription'),
  },
  approvals: {
    title: t('inspector.approvalsTitle'),
    description: t('inspector.approvalsDescription'),
  },
  changes: {
    title: t('inspector.changesTitle'),
    description: t('inspector.changesDescription'),
  },
})[activeTab.value])

function selectTab(id: string): void {
  if (id === 'context' || id === 'approvals' || id === 'changes') activeTab.value = id
}
</script>

<template>
  <div class="inspector-dock">
    <AppTabs
      :model-value="activeTab"
      :tabs="tabs"
      :ariaLabel="t('inspector.label')"
      @update:model-value="selectTab"
    />
    <div class="inspector-dock__panel" role="tabpanel" data-inspector-panel>
      <slot :name="activeTab">
        <EmptyState :title="panel.title" :description="panel.description" />
      </slot>
    </div>
  </div>
</template>

<style scoped>
.inspector-dock {
  display: flex;
  min-block-size: 100%;
  flex-direction: column;
}

.inspector-dock__panel {
  min-block-size: 0;
  flex: 1;
  overflow: auto;
  padding: var(--space-3);
}
</style>
