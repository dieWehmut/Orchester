<script setup lang="ts">
import { Network } from '@lucide/vue'
import { computed } from 'vue'

const props = defineProps<{ iconKey: string }>()

const icons: Record<string, { dark: string; light: string }> = {
  codex: { dark: 'codex.png', light: 'codex-light.png' },
  claude: { dark: 'claude.svg', light: 'claude.svg' },
  deepseek: { dark: 'deepseek.ico', light: 'deepseek.ico' },
  opencode: { dark: 'opencode-dark.svg', light: 'opencode-light.svg' },
}
const renderedKey = computed(() => (icons[props.iconKey] ? props.iconKey : 'generic'))
const icon = computed(() => icons[props.iconKey])
</script>

<template>
  <span class="agent-icon" :data-agent-icon="renderedKey" aria-hidden="true">
    <template v-if="icon">
      <img class="agent-icon__dark" :src="`/agents/${icon.dark}`" alt="" draggable="false" />
      <img class="agent-icon__light" :src="`/agents/${icon.light}`" alt="" draggable="false" />
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
