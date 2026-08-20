<script setup lang="ts">
import type { AgentFleetSnapshotDto, AgentRuntimeSummaryDto } from '@orchester/protokoll'
import { AppBadge, InlineAlert, SkeletonBlock, StatusDot } from '@orchester/design'
import { Bot, BrainCircuit, Code2, Network, Sparkles } from '@lucide/vue'
import { markRaw, type Component } from 'vue'

import type { AgentFleetStoreStatus } from '../../stores/agent-fleet'

const props = withDefaults(
  defineProps<{
    status: AgentFleetStoreStatus
    snapshot: AgentFleetSnapshotDto | null
    error?: string | null
  }>(),
  { error: null },
)

defineEmits<{
  select: [agentId: string]
}>()

const icons: Record<string, Component> = {
  codex: markRaw(Bot),
  claude: markRaw(Sparkles),
  deepseek: markRaw(BrainCircuit),
  opencode: markRaw(Code2),
  generic: markRaw(Network),
}

const genericIcon = markRaw(Network)

function iconFor(agent: AgentRuntimeSummaryDto): Component {
  return icons[agent.icon_key] ?? genericIcon
}

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
  <section class="agent-fleet" data-agent-fleet aria-labelledby="agent-fleet-title">
    <header class="agent-fleet__header">
      <div>
        <h2 id="agent-fleet-title">Agents</h2>
        <p>Runtime fleet</p>
      </div>
      <AppBadge tone="neutral" mono>{{ props.snapshot?.agents.length ?? 0 }}</AppBadge>
    </header>

    <div v-if="props.status === 'loading' && !props.snapshot" class="agent-fleet__loading" role="status">
      <SkeletonBlock :lines="4" height="2.6rem" />
    </div>

    <InlineAlert
      v-else-if="props.status === 'error' && !props.snapshot"
      tone="error"
      title="Agent status unavailable"
    >
      {{ props.error || 'The runtime did not return an agent snapshot.' }}
    </InlineAlert>

    <div v-else>
      <InlineAlert
        v-if="props.status === 'stale' && props.error"
        class="agent-fleet__stale"
        data-agent-fleet-stale
        tone="warning"
        title="Showing last known status"
      >
        {{ props.error }}
      </InlineAlert>

      <ul class="agent-fleet__list">
        <li v-for="agent in props.snapshot?.agents ?? []" :key="agent.agent_id" :data-agent-id="agent.agent_id">
          <button
            class="agent-fleet__row"
            type="button"
            :aria-label="`${agent.display_name}, ${activityLabel(agent)}`"
            @click="$emit('select', agent.agent_id)"
          >
            <span class="agent-fleet__icon" :data-agent-icon="agent.icon_key" aria-hidden="true">
              <component :is="iconFor(agent)" :size="17" :stroke-width="1.8" />
            </span>
            <span class="agent-fleet__identity">
              <strong>{{ agent.display_name }}</strong>
              <span>{{ agent.provider }}</span>
            </span>
            <span class="agent-fleet__state">
              <span class="agent-fleet__activity">
                <StatusDot :status="dotStatus(agent)" :label="activityLabel(agent)" :pulse="agent.activity === 'running'" />
                <span data-agent-activity>{{ activityLabel(agent) }}</span>
              </span>
              <span class="agent-fleet__counts" aria-label="Active windows and subagents">
                <span class="agent-fleet__count" title="Orchester-managed windows">
                  <span class="agent-fleet__count-label">windows</span>
                  <strong data-active-windows>{{ agent.active_windows }}</strong>
                </span>
                <span v-if="agent.active_subagents > 0" class="agent-fleet__count" title="Running subagents">
                  <span class="agent-fleet__count-label">subagents</span>
                  <strong data-active-subagents>{{ agent.active_subagents }}</strong>
                </span>
              </span>
            </span>
          </button>
        </li>
      </ul>
    </div>
  </section>
</template>

<style scoped>
.agent-fleet {
  display: grid;
  gap: var(--space-3);
  padding: var(--space-3);
}

.agent-fleet__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
}

.agent-fleet__header h2,
.agent-fleet__header p {
  margin: 0;
}

.agent-fleet__header h2 {
  color: var(--color-text-primary);
  font-size: var(--text-sm);
  font-weight: var(--weight-semibold);
}

.agent-fleet__header p {
  margin-block-start: 2px;
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
}

.agent-fleet__list {
  display: grid;
  gap: var(--space-1);
  margin: 0;
  padding: 0;
  list-style: none;
}

.agent-fleet__row {
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

.agent-fleet__row:hover,
.agent-fleet__row:focus-visible {
  border-color: var(--color-border-base);
  background: var(--color-bg-element);
  outline: none;
}

.agent-fleet__icon {
  display: grid;
  inline-size: 28px;
  block-size: 28px;
  place-items: center;
  border: 1px solid var(--color-border-base);
  border-radius: var(--radius-sm);
  background: var(--color-bg-element);
  color: var(--color-accent);
}

.agent-fleet__identity,
.agent-fleet__state,
.agent-fleet__activity,
.agent-fleet__counts,
.agent-fleet__count {
  display: flex;
  min-inline-size: 0;
  align-items: center;
}

.agent-fleet__identity {
  flex-direction: column;
  align-items: flex-start;
  gap: 2px;
}

.agent-fleet__identity strong {
  overflow: hidden;
  inline-size: 100%;
  color: var(--color-text-primary);
  font-size: var(--text-sm);
  font-weight: var(--weight-medium);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.agent-fleet__identity > span,
.agent-fleet__count-label {
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
}

.agent-fleet__state {
  flex-direction: column;
  align-items: flex-end;
  gap: 3px;
}

.agent-fleet__activity,
.agent-fleet__counts {
  gap: var(--space-1);
}

.agent-fleet__activity {
  color: var(--color-text-secondary);
  font-size: var(--text-xs);
  white-space: nowrap;
}

.agent-fleet__counts {
  justify-content: flex-end;
  gap: var(--space-2);
}

.agent-fleet__count {
  gap: 3px;
  font-family: var(--font-mono);
  font-size: var(--text-xs);
}

.agent-fleet__count strong {
  color: var(--color-text-secondary);
  font-weight: var(--weight-medium);
}

.agent-fleet__stale {
  margin-block-end: var(--space-2);
}
</style>
