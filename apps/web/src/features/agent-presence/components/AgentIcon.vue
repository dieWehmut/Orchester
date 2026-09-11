<script setup lang="ts">
import { Bot, BrainCircuit, Code2, Network, Sparkles } from '@lucide/vue'
import { computed, markRaw, type Component } from 'vue'

const props = defineProps<{ iconKey: string }>()

const icons: Record<string, Component> = {
  codex: markRaw(Bot),
  claude: markRaw(Sparkles),
  deepseek: markRaw(BrainCircuit),
  opencode: markRaw(Code2),
}
const fallbackIcon = markRaw(Network)
const renderedKey = computed(() => (icons[props.iconKey] ? props.iconKey : 'generic'))
const renderedIcon = computed<Component>(() => icons[props.iconKey] ?? fallbackIcon)
</script>

<template>
  <span class="agent-icon" :data-agent-icon="renderedKey" aria-hidden="true">
    <component :is="renderedIcon" :size="17" :stroke-width="1.8" />
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
</style>
