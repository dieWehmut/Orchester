<script setup lang="ts">
import type { AgentRuntimeSummaryDto } from '@orchester/protokoll'
import { StatusDot } from '@orchester/design'

import AgentIcon from './AgentIcon.vue'

const props = defineProps<{ agent: AgentRuntimeSummaryDto }>()

defineEmits<{
  select: [agentId: string]
}>()

function activityLabel(agent: AgentRuntimeSummaryDto): string {
  if (agent.availability === 'auth_required') return 'Sign in required'
  if (agent.availability === 'unavailable') return 'Unavailable'
  if (agent.availability === 'error') return 'Error'
  switch (agent.activity) {
    case 'running':
      return 'Running'
    case 'waiting_approval':
      return 'Waiting approval'
    case 'starting':
      return 'Starting'
    case 'stopping':
      return 'Stopping'
    case 'idle':
      return 'Idle'
    case 'offline':
      return 'Offline'
    case 'error':
      return 'Error'
  }
}

function dotStatus(agent: AgentRuntimeSummaryDto): 'idle' | 'running' | 'waiting' | 'success' | 'error' {
  if (agent.availability === 'auth_required' || agent.activity === 'waiting_approval') return 'waiting'
  if (agent.availability === 'unavailable' || agent.activity === 'offline') return 'idle'
  if (agent.availability === 'error' || agent.activity === 'error') return 'error'
  if (agent.activity === 'running' || agent.activity === 'starting' || agent.activity === 'stopping') return 'running'
  return 'success'
}
</script>

<template>
  <button
    class="agent-fleet-row"
    type="button"
    :aria-label="`${props.agent.display_name}, ${activityLabel(props.agent)}`"
    @click="$emit('select', props.agent.agent_id)"
  >
    <AgentIcon :icon-key="props.agent.icon_key" />
    <span class="agent-fleet-row__identity">
      <strong>{{ props.agent.display_name }}</strong>
      <span>{{ props.agent.provider }}</span>
    </span>
    <span class="agent-fleet-row__state">
      <span class="agent-fleet-row__activity">
        <StatusDot
          :status="dotStatus(props.agent)"
          :label="activityLabel(props.agent)"
          :pulse="props.agent.activity === 'running'"
        />
        <span data-agent-activity>{{ activityLabel(props.agent) }}</span>
      </span>
      <span class="agent-fleet-row__counts" aria-label="Active windows and subagents">
        <span class="agent-fleet-row__count" title="Orchester-managed windows">
          <span class="agent-fleet-row__count-label">windows</span>
          <strong data-active-windows>{{ props.agent.active_windows }}</strong>
        </span>
        <span
          v-if="props.agent.active_subagents > 0"
          class="agent-fleet-row__count"
          title="Running subagents"
        >
          <span class="agent-fleet-row__count-label">subagents</span>
          <strong data-active-subagents>{{ props.agent.active_subagents }}</strong>
        </span>
      </span>
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

.agent-fleet-row__identity,
.agent-fleet-row__state,
.agent-fleet-row__activity,
.agent-fleet-row__counts,
.agent-fleet-row__count {
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

.agent-fleet-row__identity > span,
.agent-fleet-row__count-label {
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
}

.agent-fleet-row__state {
  flex-direction: column;
  align-items: flex-end;
  gap: 3px;
}

.agent-fleet-row__activity,
.agent-fleet-row__counts {
  gap: var(--space-1);
}

.agent-fleet-row__activity {
  color: var(--color-text-secondary);
  font-size: var(--text-xs);
  white-space: nowrap;
}

.agent-fleet-row__counts {
  justify-content: flex-end;
  gap: var(--space-2);
}

.agent-fleet-row__count {
  gap: 3px;
  font-family: var(--font-mono);
  font-size: var(--text-xs);
}

.agent-fleet-row__count strong {
  color: var(--color-text-secondary);
  font-weight: var(--weight-medium);
}
</style>
