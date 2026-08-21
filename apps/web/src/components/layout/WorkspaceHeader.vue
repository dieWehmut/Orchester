<script setup lang="ts">
import { StatusDot, ThemeToggle } from '@orchester/design'
import { computed } from 'vue'

import { useI18n } from '../../i18n'

export type RuntimeConnection = 'pending' | 'ready' | 'offline' | 'error'

const props = withDefaults(
  defineProps<{
    connection?: RuntimeConnection
    workspaceName?: string | null
  }>(),
  { connection: 'pending', workspaceName: null },
)

const { t } = useI18n()
const connectionLabel = computed(() => {
  if (props.connection === 'ready') return t('app.connected')
  if (props.connection === 'offline') return t('app.offline')
  if (props.connection === 'error') return t('app.connectionError')
  return t('app.runtimePending')
})
const connectionStatus = computed(() => {
  if (props.connection === 'ready') return 'success' as const
  if (props.connection === 'error') return 'error' as const
  if (props.connection === 'pending') return 'running' as const
  return 'idle' as const
})
</script>

<template>
  <header class="workspace-header">
    <div class="workspace-header__brand">
      <div class="workspace-header__mark" aria-hidden="true">O</div>
      <div class="workspace-header__identity">
        <strong data-testid="product-name">{{ t('app.name') }}</strong>
        <span data-testid="workspace-name">{{ workspaceName || t('app.unknownWorkspace') }}</span>
      </div>
    </div>

    <div class="workspace-header__actions">
      <slot name="actions" />
      <div class="workspace-header__connection">
        <StatusDot
          :status="connectionStatus"
          :label="connectionLabel"
          :pulse="connection === 'pending'"
        />
        <span data-testid="connection-label">{{ connectionLabel }}</span>
      </div>
      <ThemeToggle />
    </div>
  </header>
</template>

<style scoped>
.workspace-header {
  position: sticky;
  inset-block-start: 0;
  z-index: var(--z-header);
  display: flex;
  min-block-size: var(--header-height);
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
  padding-inline: var(--space-4);
  border-block-end: 1px solid var(--color-border-base);
  background: color-mix(in srgb, var(--color-bg-surface) 94%, transparent);
  backdrop-filter: blur(12px);
}

.workspace-header__brand,
.workspace-header__actions,
.workspace-header__connection {
  display: flex;
  align-items: center;
}

.workspace-header__brand {
  min-inline-size: 0;
  gap: var(--space-3);
}

.workspace-header__mark {
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

.workspace-header__identity {
  display: grid;
  min-inline-size: 0;
  line-height: var(--leading-tight);
}

.workspace-header__identity strong {
  font-size: var(--text-sm);
}

.workspace-header__identity span,
.workspace-header__connection {
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
}

.workspace-header__identity span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.workspace-header__actions {
  min-inline-size: 0;
  gap: var(--space-2);
}

.workspace-header__connection {
  min-block-size: var(--control-height-sm);
  gap: var(--space-2);
  padding-inline: var(--space-3);
  border: 1px solid var(--color-border-base);
  border-radius: var(--radius-sm);
  background: var(--color-bg-element);
  white-space: nowrap;
}

@media (max-width: 640px) {
  .workspace-header__connection span,
  .workspace-header__identity span {
    display: none;
  }

  .workspace-header__connection {
    inline-size: var(--control-height-sm);
    justify-content: center;
    padding: 0;
  }
}
</style>
