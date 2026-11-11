<script setup lang="ts">
import { Network } from '@lucide/vue'
import { computed } from 'vue'
import { agentIdentity } from '../agent-identities'

const props = defineProps<{ iconKey: string }>()
const identity = computed(() => agentIdentity(props.iconKey))
</script>

<template>
  <span class="agent-icon" :data-agent-icon="identity?.key ?? 'generic'" aria-hidden="true">
    <template v-if="identity">
      <img class="agent-icon__dark" :src="identity.marks.dark" alt="" draggable="false" />
      <img class="agent-icon__light" :src="identity.marks.light" alt="" draggable="false" />
    </template>
    <Network v-else :size="17" :stroke-width="1.8" />
  </span>
</template>

<style scoped>
.agent-icon {
  display: grid;
  inline-size: 28px;
  block-size: 28px;
  place-items: center;
  border: 1px solid var(--color-border-base);
  border-radius: var(--radius-sm);
  background: var(--color-bg-element);
  color: var(--color-accent);
}
.agent-icon img { inline-size: 21px; block-size: 21px; object-fit: contain; }
.agent-icon .agent-icon__light { display: none; }
:global([data-theme='light'] .agent-icon .agent-icon__dark) { display: none; }
:global([data-theme='light'] .agent-icon .agent-icon__light) { display: block; }
</style>
