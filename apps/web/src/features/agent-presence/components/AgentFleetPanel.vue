<script setup lang="ts">
import type { AgentFleetSnapshotDto } from '@orchester/protokoll'
import { AppBadge, InlineAlert, SkeletonBlock } from '@orchester/design'
import { computed } from 'vue'

import { useI18n } from '../../../i18n'
import type { AgentFleetStoreStatus } from '../../../stores/agent-fleet'
import type { AgentStatusSocketStatus } from '../../../transport/agent-status-socket'
import { agentStreamStatusMessageKey } from '../agent-presenter'
import { groupAgentFleet } from '../fleet-groups'
import AgentFleetGroup from './AgentFleetGroup.vue'

const props = withDefaults(
  defineProps<{
    status: AgentFleetStoreStatus
    streamStatus?: AgentStatusSocketStatus
    snapshot: AgentFleetSnapshotDto | null
    error?: string | null
    selectedAgentId?: string | null
  }>(),
  { error: null, streamStatus: 'idle', selectedAgentId: null },
)

defineEmits<{
  select: [agentId: string]
}>()

const { t } = useI18n()
const streamStatusLabel = computed(() => t(agentStreamStatusMessageKey(props.streamStatus)))
const groups = computed(() => groupAgentFleet(props.snapshot?.agents ?? []))

function agentStreamStatusTone(status: AgentStatusSocketStatus): 'neutral' | 'success' | 'warning' | 'error' {
  if (status === 'connected') return 'success'
  if (status === 'reconnecting' || status === 'connecting') return 'warning'
  if (status === 'fatal') return 'error'
  return 'neutral'
}
</script>

<template>
  <section class="agent-fleet" data-agent-fleet :aria-label="t('agents.title')">
    <header class="agent-fleet__header">
      <span class="agent-fleet__subtitle">{{ t('agents.runtimeFleet') }}</span>
      <div class="agent-fleet__header-meta">
        <AppBadge
          data-agent-stream-status
          :tone="agentStreamStatusTone(props.streamStatus)"
          mono
        >
          {{ streamStatusLabel }}
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
      :title="t('agents.statusUnavailable')"
    >
      {{ props.error || t('agents.runtimeSnapshotMissing') }}
    </InlineAlert>

    <div v-else>
      <InlineAlert
        v-if="props.status === 'stale' && props.error"
        class="agent-fleet__stale"
        data-agent-fleet-stale
        tone="warning"
        :title="t('agents.showingLastKnown')"
      >
        {{ props.error }}
      </InlineAlert>

      <div class="agent-fleet__groups">
        <AgentFleetGroup
          v-for="group in groups"
          :key="group.key"
          :group="group"
          :selected-agent-id="props.selectedAgentId"
          @select="$emit('select', $event)"
        />
      </div>
    </div>
  </section>
</template>

<style scoped>
.agent-fleet {
  display: grid;
  gap: var(--space-3);
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

/* The fleet's own state, under the heading the rail draws: the name belongs to
   the column, the status and the count belong to the list. */
.agent-fleet__subtitle {
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
}

.agent-fleet__groups {
  display: grid;
  gap: var(--space-2);
}

.agent-fleet__stale {
  margin-block-end: var(--space-2);
}
</style>
