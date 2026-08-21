<script setup lang="ts">
import type { AgentRuntimeSummaryDto } from '@orchester/protokoll'
import { StatusDot } from '@orchester/design'
import { computed } from 'vue'
import { useI18n } from '../../../i18n'

import AgentProviderMark from './AgentProviderMark.vue'
import AgentMetrics from './AgentMetrics.vue'
import {
  activeAgentCounts,
  agentActivityMessageKey,
  agentCountMessageKey,
  agentDotStatus,
} from '../agent-presenter'
import { agentProviderPresentation } from '../provider-presentation'

const props = withDefaults(
  defineProps<{
    agent: AgentRuntimeSummaryDto
    selected?: boolean
  }>(),
  { selected: false },
)
const { t } = useI18n()
const provider = computed(() => agentProviderPresentation(props.agent))
const activityLabel = computed(() => t(agentActivityMessageKey(props.agent)))
const metricSummary = computed(() =>
  activeAgentCounts(props.agent)
    .map((entry) => `${entry.count} ${t(agentCountMessageKey(entry.key, entry.count))}`)
    .join(', '),
)

defineEmits<{
  select: [agentId: string]
}>()
</script>

<template>
  <button
    class="agent-fleet-row"
    :class="{ 'agent-fleet-row--selected': props.selected }"
    type="button"
    :aria-pressed="props.selected"
    :aria-label="`${props.agent.display_name}, ${provider.label}, ${activityLabel}, ${metricSummary}`"
    @click="$emit('select', props.agent.agent_id)"
  >
    <AgentProviderMark :agent="props.agent" />
    <span class="agent-fleet-row__identity">
      <strong>{{ props.agent.display_name }}</strong>
      <span data-agent-provider-label>{{ provider.label }}</span>
    </span>
    <span class="agent-fleet-row__state">
      <span class="agent-fleet-row__activity">
        <StatusDot
          :status="agentDotStatus(props.agent)"
          :label="activityLabel"
          :pulse="props.agent.activity === 'running'"
        />
        <span data-agent-activity>{{ activityLabel }}</span>
      </span>
      <AgentMetrics :agent="props.agent" variant="compact" />
    </span>
  </button>
</template>

<style scoped>
.agent-fleet-row {
  display: grid;
  grid-template-columns: 28px minmax(0, 1fr) auto;
  align-items: center;
  inline-size: 100%;
  gap: var(--space-2);
  padding: var(--space-2);
  border: 1px solid transparent;
  border-radius: var(--radius-sm);
  background: transparent;
  color: inherit;
  font: inherit;
  text-align: start;
  cursor: pointer;
}

.agent-fleet-row:hover,
.agent-fleet-row:focus-visible {
  border-color: var(--color-border-base);
  background: var(--color-bg-element);
  outline: none;
}

.agent-fleet-row--selected {
  border-color: var(--color-accent-border);
  background: var(--color-accent-muted);
}

.agent-fleet-row__identity,
.agent-fleet-row__state,
.agent-fleet-row__activity {
  display: flex;
  min-inline-size: 0;
  align-items: center;
}

.agent-fleet-row__identity {
  flex-direction: column;
  align-items: flex-start;
  gap: 2px;
}

.agent-fleet-row__identity strong {
  overflow: hidden;
  inline-size: 100%;
  color: var(--color-text-primary);
  font-size: var(--text-sm);
  font-weight: var(--weight-medium);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.agent-fleet-row__identity > span {
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
}

.agent-fleet-row__state {
  flex-direction: column;
  align-items: flex-end;
  gap: 3px;
}

.agent-fleet-row__activity {
  gap: var(--space-1);
}

.agent-fleet-row__activity {
  color: var(--color-text-secondary);
  font-size: var(--text-xs);
  white-space: nowrap;
}

</style>
