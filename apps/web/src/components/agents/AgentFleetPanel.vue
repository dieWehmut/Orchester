<script setup lang="ts">
import type { AgentFleetSnapshotDto } from '@orchester/protokoll'
import { AppBadge, InlineAlert, SkeletonBlock } from '@orchester/design'

import type { AgentFleetStoreStatus } from '../../stores/agent-fleet'
import type { AgentStatusSocketStatus } from '../../transport/agent-status-socket'
import AgentFleetRow from './AgentFleetRow.vue'

const props = withDefaults(
  defineProps<{
    status: AgentFleetStoreStatus
    streamStatus?: AgentStatusSocketStatus
    snapshot: AgentFleetSnapshotDto | null
    error?: string | null
  }>(),
  { error: null, streamStatus: 'idle' },
)

defineEmits<{
  select: [agentId: string]
}>()

</script>

<script lang="ts">
export function agentStreamStatusLabel(status: AgentStatusSocketStatus): string {
  switch (status) {
    case 'connected':
      return 'Live'
    case 'connecting':
      return 'Connecting'
    case 'reconnecting':
      return 'Reconnecting'
    case 'fatal':
      return 'Offline'
    case 'closed':
      return 'Stopped'
    case 'idle':
      return 'Not connected'
  }
}

export function agentStreamStatusTone(
  status: AgentStatusSocketStatus,
): 'neutral' | 'success' | 'warning' | 'error' {
  if (status === 'connected') return 'success'
  if (status === 'reconnecting' || status === 'connecting') return 'warning'
  if (status === 'fatal') return 'error'
  return 'neutral'
}
</script>

<template>
  <section class="agent-fleet" data-agent-fleet aria-labelledby="agent-fleet-title">
    <header class="agent-fleet__header">
      <div>
        <h2 id="agent-fleet-title">Agents</h2>
        <p>Runtime fleet</p>
      </div>
      <div class="agent-fleet__header-meta">
        <AppBadge
          data-agent-stream-status
          :tone="agentStreamStatusTone(props.streamStatus)"
          mono
        >
          {{ agentStreamStatusLabel(props.streamStatus) }}
        </AppBadge>
        <AppBadge tone="neutral" mono>{{ props.snapshot?.agents.length ?? 0 }}</AppBadge>
      </div>
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
          <AgentFleetRow :agent="agent" @select="$emit('select', $event)" />
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

.agent-fleet__header-meta {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: var(--space-2);
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

.agent-fleet__stale {
  margin-block-end: var(--space-2);
}
</style>
