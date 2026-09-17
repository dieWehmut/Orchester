<script setup lang="ts">
import { SquarePen } from '@lucide/vue'

import { AppButton } from '@orchester/design'

withDefaults(
  defineProps<{
    productName: string
    workspaceName?: string | null
    newSessionLabel: string
    projectsLabel: string
    sessionsLabel: string
    fleetLabel: string
  }>(),
  { workspaceName: null },
)

defineEmits<{
  newSession: []
}>()
</script>

<template>
  <div class="app-rail" data-app-rail>
    <div class="app-rail__section" data-rail-section="brand">
      <span class="app-rail__mark" aria-hidden="true">{{ productName.slice(0, 1) }}</span>
      <span class="app-rail__identity">
        <strong>{{ productName }}</strong>
        <span v-if="workspaceName">{{ workspaceName }}</span>
      </span>
    </div>

    <div class="app-rail__section" data-rail-section="primary">
      <AppButton
        block
        variant="secondary"
        data-rail-action="new-session"
        @click="$emit('newSession')"
      >
        <SquarePen :size="16" aria-hidden="true" />
        {{ newSessionLabel }}
      </AppButton>
    </div>

    <div class="app-rail__section app-rail__section--scroll" data-rail-section="projects">
      <h2 class="app-rail__heading">{{ projectsLabel }}</h2>
      <slot name="projects" />
    </div>

    <div class="app-rail__section app-rail__section--scroll" data-rail-section="sessions">
      <h2 class="app-rail__heading">{{ sessionsLabel }}</h2>
      <slot name="sessions" />
    </div>

    <div class="app-rail__section app-rail__section--footer" data-rail-section="fleet">
      <h2 class="app-rail__heading">{{ fleetLabel }}</h2>
      <slot name="fleet" />
    </div>
  </div>
</template>

<style scoped>
.app-rail {
  display: grid;
  grid-template-rows: auto auto minmax(0, 1fr) minmax(0, 1fr) auto;
  block-size: 100%;
  min-block-size: 0;
  background: var(--color-bg-surface);
}

.app-rail__section {
  min-inline-size: 0;
  padding: var(--space-3);
}

.app-rail__section--scroll {
  min-block-size: 0;
  overflow: auto;
}

.app-rail__section--footer {
  border-block-start: 1px solid var(--color-border-base);
  background: color-mix(in srgb, var(--color-bg-surface) 94%, var(--color-bg-base));
}

.app-rail__section[data-rail-section='brand'] {
  display: flex;
  min-inline-size: 0;
  align-items: center;
  gap: var(--space-3);
  border-block-end: 1px solid var(--color-border-base);
}

.app-rail__mark {
  display: grid;
  inline-size: 32px;
  block-size: 32px;
  flex: 0 0 32px;
  place-items: center;
  border: 1px solid var(--color-accent-border);
  border-radius: var(--radius-sm);
  background: var(--color-accent-muted);
  color: var(--color-accent);
  font-family: var(--font-mono);
  font-weight: var(--weight-semibold);
}

.app-rail__identity {
  display: grid;
  min-inline-size: 0;
  line-height: var(--leading-tight);
}

.app-rail__identity strong {
  font-size: var(--text-sm);
}

.app-rail__identity span {
  overflow: hidden;
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.app-rail__heading {
  margin: 0 0 var(--space-2);
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
  font-weight: var(--weight-semibold);
  letter-spacing: 0.04em;
  text-transform: uppercase;
}
</style>
