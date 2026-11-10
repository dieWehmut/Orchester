<script setup lang="ts">
import { AppBadge } from '@orchester/design'
import { computed } from 'vue'

import { useI18n, type MessageKey } from '../../../i18n'
import type { AgentFleetGroup } from '../fleet-groups'
import { agentCountMessageKey, agentCountValue, agentFleetCountKey } from '../agent-presenter'
import AgentFleetRow from './AgentFleetRow.vue'

const props = withDefaults(
  defineProps<{
    group: AgentFleetGroup
    selectedAgentId?: string | null
  }>(),
  { selectedAgentId: null },
)

defineEmits<{
  select: [agentId: string]
}>()

const { t } = useI18n()
const titleId = computed(() => `agent-fleet-group-${props.group.key}`)
const titleKey = computed<MessageKey>(() => `agents.groups.${props.group.key}`)
const agentCountLabel = computed(
  () => `${props.group.agents.length} ${t('agents.groupCountLabel')}`,
)
const windowValue = computed(() => agentCountValue({ count: props.group.activeWindows, quality: props.group.activeWindowQuality }, t))
const countKey = computed(() => agentFleetCountKey(props.group.agents))
const countUnit = computed(() => t(agentCountMessageKey(countKey.value, props.group.activeWindows ?? 2)))
const windowLabel = computed(() => `${props.group.activeWindows === null ? t('agents.activity.unknown') : windowValue.value} ${countUnit.value}`)
</script>

<template>
  <section
    class="agent-fleet-group"
    :data-agent-group="props.group.key"
    :aria-labelledby="titleId"
  >
    <header class="agent-fleet-group__header">
      <h3 :id="titleId">{{ t(titleKey) }}</h3>
      <span class="agent-fleet-group__meta">
        <AppBadge tone="neutral" mono :aria-label="agentCountLabel">
          {{ props.group.agents.length }}
        </AppBadge>
        <AppBadge
          v-if="props.group.activeWindows === null || props.group.activeWindows > 0"
          data-agent-group-windows
          :data-agent-count-unit="countKey"
          tone="neutral"
          mono
          :aria-label="windowLabel"
        >
          {{ windowValue }} {{ countUnit }}
        </AppBadge>
      </span>
    </header>

    <ul class="agent-fleet-group__list">
      <li
        v-for="agent in props.group.agents"
        :key="agent.agent_id"
        :data-agent-id="agent.agent_id"
      >
        <AgentFleetRow
          :agent="agent"
          :selected="agent.agent_id === props.selectedAgentId"
          @select="$emit('select', $event)"
        />
      </li>
    </ul>
  </section>
</template>

<style scoped>
.agent-fleet-group {
  display: grid;
  gap: var(--space-1);
}

.agent-fleet-group + .agent-fleet-group {
  padding-block-start: var(--space-2);
}

.agent-fleet-group__header,
.agent-fleet-group__meta {
  display: flex;
  align-items: center;
}

.agent-fleet-group__header {
  min-block-size: 1.5rem;
  justify-content: space-between;
  gap: var(--space-2);
  padding-inline: var(--space-2);
}

.agent-fleet-group__header h3 {
  margin: 0;
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
  font-weight: var(--weight-medium);
}

.agent-fleet-group__meta {
  justify-content: flex-end;
  gap: var(--space-1);
}

.agent-fleet-group__list {
  display: grid;
  gap: var(--space-1);
  margin: 0;
  padding: 0;
  list-style: none;
}
</style>
