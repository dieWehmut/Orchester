<script setup lang="ts">
import type { AgentRuntimeSummaryDto } from '@orchester/protokoll'
import { AppBadge, EmptyState, StatusDot } from '@orchester/design'
import { computed } from 'vue'

import { useI18n } from '../../../i18n'
import AgentProviderMark from './AgentProviderMark.vue'
import {
  activeAgentCounts,
  agentActivityMessageKey,
  agentAvailabilityMessageKey,
  agentDotStatus,
  agentCountMessageKey,
} from '../agent-presenter'
import { agentProviderPresentation } from '../provider-presentation'

const props = defineProps<{
  agent: AgentRuntimeSummaryDto | null
}>()

const { t } = useI18n()
const activityLabel = computed(() => (props.agent ? t(agentActivityMessageKey(props.agent)) : ''))
const availabilityLabel = computed(() =>
  props.agent ? t(agentAvailabilityMessageKey(props.agent)) : '',
)
const provider = computed(() => (props.agent ? agentProviderPresentation(props.agent) : null))
</script>

<template>
  <section v-if="props.agent" class="agent-details" data-agent-details>
    <header class="agent-details__header">
      <span :data-agent-details-icon="props.agent.icon_key">
        <AgentProviderMark :agent="props.agent" />
      </span>
      <div class="agent-details__identity">
        <strong data-agent-details-name>{{ props.agent.display_name }}</strong>
        <span data-agent-provider-label>{{ provider?.label }}</span>
      </div>
      <StatusDot
        :status="agentDotStatus(props.agent)"
        :label="activityLabel"
        :pulse="props.agent.activity === 'running'"
      />
      <AppBadge data-agent-details-activity :tone="agentDotStatus(props.agent) === 'error' ? 'error' : 'neutral'">
        {{ activityLabel }}
      </AppBadge>
    </header>

    <div class="agent-details__availability" data-agent-details-availability>
      <span>{{ t('agents.availabilityLabel') }}</span>
      <strong>{{ availabilityLabel }}</strong>
    </div>

    <dl class="agent-details__metrics">
      <div v-for="entry in activeAgentCounts(props.agent)" :key="entry.key" :data-agent-detail="entry.key">
        <dt>{{ t(agentCountMessageKey(entry.key)) }}</dt>
        <dd data-agent-detail-value>{{ entry.count }}</dd>
      </div>
    </dl>

    <section class="agent-details__capabilities" aria-labelledby="agent-capabilities-title">
      <h3 id="agent-capabilities-title">{{ t('agents.capabilities') }}</h3>
      <div data-agent-capabilities>
        <AppBadge v-for="capability in props.agent.capabilities" :key="capability" tone="neutral" mono>
          {{ capability }}
        </AppBadge>
        <span v-if="props.agent.capabilities.length === 0" class="agent-details__muted">
          {{ t('agents.noCapabilities') }}
        </span>
      </div>
    </section>

    <p v-if="props.agent.last_error" class="agent-details__error" data-agent-details-error>
      {{ props.agent.last_error }}
    </p>
  </section>

  <div v-else data-agent-details-empty>
    <EmptyState :title="t('agents.noSelection')" />
  </div>
</template>

<style scoped>
.agent-details {
  display: grid;
  gap: var(--space-4);
  min-block-size: 100%;
}

.agent-details__header {
  display: grid;
  grid-template-columns: 2rem minmax(0, 1fr) auto auto;
  align-items: center;
  gap: var(--space-2);
  padding-block-end: var(--space-3);
  border-block-end: 1px solid var(--color-border-base);
}

.agent-details__identity {
  display: grid;
  min-inline-size: 0;
  gap: 2px;
}

.agent-details__identity strong {
  overflow: hidden;
  color: var(--color-text-primary);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.agent-details__identity span,
.agent-details__availability,
.agent-details__muted {
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
}

.agent-details__availability {
  display: flex;
  justify-content: space-between;
  gap: var(--space-3);
}

.agent-details__availability strong {
  color: var(--color-text-secondary);
  font-weight: var(--weight-medium);
}

.agent-details__metrics {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--space-2);
  margin: 0;
}

.agent-details__metrics > div {
  min-inline-size: 0;
  padding: var(--space-2);
  border: 1px solid var(--color-border-base);
  border-radius: var(--radius-sm);
  background: var(--color-bg-element);
}

.agent-details__metrics dt {
  overflow: hidden;
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.agent-details__metrics dd {
  margin: var(--space-1) 0 0;
  color: var(--color-text-primary);
  font-family: var(--font-mono);
  font-size: var(--text-lg);
}

.agent-details__capabilities {
  display: grid;
  gap: var(--space-2);
}

.agent-details__capabilities h3 {
  margin: 0;
  color: var(--color-text-secondary);
  font-size: var(--text-sm);
  font-weight: var(--weight-medium);
}

.agent-details__capabilities > div {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-1);
}

.agent-details__error {
  margin: 0;
  padding: var(--space-2);
  border-inline-start: 2px solid var(--color-status-error);
  color: var(--color-status-error);
  font-size: var(--text-xs);
  overflow-wrap: anywhere;
}
</style>
