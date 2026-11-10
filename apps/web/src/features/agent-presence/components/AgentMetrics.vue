<script setup lang="ts">
import type { AgentRuntimeSummaryDto } from '@orchester/protokoll'
import { computed } from 'vue'

import { useI18n } from '../../../i18n'
import {
  activeAgentCounts,
  agentCountMessageKey,
  agentCountValue,
  agentCountsSummary,
  agentProcessSourcesComplete,
  agentWindowSourceMessageKey,
} from '../agent-presenter'

const props = withDefaults(
  defineProps<{
    agent: AgentRuntimeSummaryDto
    variant?: 'compact' | 'detail'
  }>(),
  { variant: 'compact' },
)

const { t } = useI18n()
const entries = computed(() => activeAgentCounts(props.agent))
const summary = computed(() => agentCountsSummary(props.agent, t))
const windowSource = computed(() =>
  t(agentWindowSourceMessageKey(props.agent.window_count_source)),
)
</script>

<template>
  <dl
    class="agent-metrics"
    :class="`agent-metrics--${props.variant}`"
    data-agent-metrics
    :aria-label="summary"
  >
    <div
      v-for="entry in entries"
      :key="entry.key"
      class="agent-metrics__item"
      data-agent-count
      :data-agent-count-unit="entry.key"
      :data-agent-detail="props.variant === 'detail' ? entry.key : undefined"
    >
      <dt>{{ t(agentCountMessageKey(entry.key, entry.count ?? 2)) }}</dt>
      <dd
        data-agent-detail-value
        :data-active-windows="entry.key === 'windows' || entry.key === 'processes' ? '' : undefined"
        :data-active-processes="entry.key === 'processes' ? '' : undefined"
        :data-active-runs="entry.key === 'runs' ? '' : undefined"
        :data-active-subagents="entry.key === 'subagents' ? '' : undefined"
      >
        {{ agentCountValue(entry, t) }}
      </dd>
    </div>
  </dl>

  <p
    v-if="props.variant === 'detail'"
    class="agent-metrics__source"
    data-agent-window-source
  >
    {{ t('agents.windowSourceLabel') }}: {{ windowSource }}
  </p>

  <div v-if="props.variant === 'detail'" class="agent-metrics__sources" data-agent-process-sources>
    <p>{{ t(agentProcessSourcesComplete(props.agent) ? 'agents.processSources.available' : 'agents.processSources.incomplete') }}</p>
    <p v-if="props.agent.process_sources === null">{{ t('agents.activity.unknown') }}</p>
    <ul v-else>
      <li v-for="(source, index) in props.agent.process_sources" :key="index" :data-process-source-status="source.status">
        {{ t(`agents.processSources.${source.environment}`) }}:
        {{ source.status === 'stale' ? t('agents.processSources.lastObserved', { count: String(source.count) }) : source.status === 'available' ? t('agents.processSources.current', { count: String(source.count) }) : t('agents.processSources.unavailable') }}
        <time v-if="source.observed_at" :datetime="source.observed_at">{{ source.observed_at }}</time>
      </li>
    </ul>
  </div>
</template>

<style scoped>
.agent-metrics {
  margin: 0;
}

.agent-metrics--compact {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: var(--space-2);
}

.agent-metrics__item {
  display: flex;
  min-inline-size: 0;
}

.agent-metrics--compact .agent-metrics__item {
  align-items: center;
  gap: var(--space-1);
  font-family: var(--font-mono);
  font-size: var(--text-xs);
}

.agent-metrics--compact dt {
  color: var(--color-text-tertiary);
}

.agent-metrics--compact dd {
  margin: 0;
  color: var(--color-text-secondary);
  font-weight: var(--weight-medium);
}

.agent-metrics--detail {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--space-2);
}

.agent-metrics--detail .agent-metrics__item {
  flex-direction: column;
  padding: var(--space-2);
  border: 1px solid var(--color-border-base);
  border-radius: var(--radius-sm);
  background: var(--color-bg-element);
}

.agent-metrics--detail dt {
  overflow: hidden;
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.agent-metrics--detail dd {
  margin: var(--space-1) 0 0;
  color: var(--color-text-primary);
  font-family: var(--font-mono);
  font-size: var(--text-lg);
}

.agent-metrics__source {
  margin: calc(var(--space-3) * -1) 0 0;
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
}

.agent-metrics__sources { color: var(--color-text-tertiary); font-size: var(--text-xs); }
.agent-metrics__sources p { margin: 0 0 var(--space-1); }
.agent-metrics__sources ul { margin: 0; padding-inline-start: var(--space-4); }
.agent-metrics__sources time { display: block; }
</style>
