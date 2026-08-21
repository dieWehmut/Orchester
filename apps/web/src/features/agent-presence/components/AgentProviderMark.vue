<script setup lang="ts">
import type { AgentRuntimeSummaryDto } from '@orchester/protokoll'
import { computed } from 'vue'

import AgentIcon from './AgentIcon.vue'
import { agentProviderPresentation } from '../provider-presentation'

const props = defineProps<{
  agent: AgentRuntimeSummaryDto
}>()

const provider = computed(() => agentProviderPresentation(props.agent))
</script>

<template>
  <span
    class="agent-provider-mark"
    :data-agent-provider="provider.key"
    :data-agent-provider-tone="provider.tone"
    :title="provider.label"
  >
    <AgentIcon :icon-key="provider.iconKey === 'generic' ? props.agent.icon_key : provider.iconKey" />
  </span>
</template>

<style scoped>
.agent-provider-mark {
  display: grid;
  inline-size: max-content;
  border-radius: var(--radius-sm);
}

.agent-provider-mark[data-agent-provider-tone='success'] {
  --agent-provider-color: var(--color-status-success);
}

.agent-provider-mark[data-agent-provider-tone='warning'] {
  --agent-provider-color: var(--color-status-warning);
}

.agent-provider-mark[data-agent-provider-tone='info'] {
  --agent-provider-color: var(--color-status-info);
}

.agent-provider-mark[data-agent-provider-tone='accent'] {
  --agent-provider-color: var(--color-accent);
}

.agent-provider-mark[data-agent-provider-tone='neutral'] {
  --agent-provider-color: var(--color-status-neutral);
}

.agent-provider-mark :deep(.agent-icon) {
  color: var(--agent-provider-color, var(--color-accent));
}
</style>
