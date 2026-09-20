<script setup lang="ts">
import { AppTabs, EmptyState, type AppTabOption } from '@orchester/design'
import { computed, ref } from 'vue'

import { useI18n } from '../../i18n'
import { isInspectorTab, type InspectorTab } from './inspector-tabs'

const props = withDefaults(
  defineProps<{
    activeTab?: InspectorTab
    /**
     * Whether the terminal lives in the inspector, section 4.7.
     *
     * The preference decides, and the dock is told rather than asking, so a
     * terminal the reader moved to the bottom panel leaves no tab here that
     * opens nothing.
     */
    terminal?: boolean
  }>(),
  { terminal: false },
)

const emit = defineEmits<{
  'update:activeTab': [value: InspectorTab]
}>()

const { t } = useI18n()
const uncontrolledActiveTab = ref<InspectorTab>('context')
const activeTab = computed<InspectorTab>({
  get: () => props.activeTab ?? uncontrolledActiveTab.value,
  set: (value) => {
    if (props.activeTab === undefined) uncontrolledActiveTab.value = value
    emit('update:activeTab', value)
  },
})
const tabs = computed<AppTabOption[]>(() => [
  { id: 'context', label: t('inspector.context') },
  { id: 'approvals', label: t('inspector.approvals') },
  { id: 'changes', label: t('inspector.review') },
  ...(props.terminal ? [{ id: 'terminal', label: t('bottomPanel.terminal') }] : []),
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
    // Section 4.7 names this tab Review: it is the working copy change set,
    // not only the changes this run reported.
    title: t('inspector.reviewTitle'),
    description: t('inspector.reviewDescription'),
  },
  terminal: {
    title: t('bottomPanel.terminal'),
    description: t('bottomPanel.terminalEmpty'),
  },
})[activeTab.value])

function selectTab(id: string): void {
  if (isInspectorTab(id)) activeTab.value = id
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
