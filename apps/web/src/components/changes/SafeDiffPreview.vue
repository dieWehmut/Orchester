<script setup lang="ts">
import { FileWarning, ScissorsLineDashed } from '@lucide/vue'
import { AppBadge, EmptyState, InlineAlert } from '@orchester/design'
import { computed } from 'vue'

import { prepareDiffText } from './safe-diff'

const props = withDefaults(
  defineProps<{
    text: string | null
    maxBytes?: number
    maxLines?: number
  }>(),
  { maxBytes: 256 * 1024, maxLines: 2_000 },
)

const prepared = computed(() =>
  props.text === null
    ? { status: 'empty' as const }
    : prepareDiffText(props.text, { maxBytes: props.maxBytes, maxLines: props.maxLines }),
)
</script>

<template>
  <section class="safe-diff-preview" aria-label="Diff preview">
    <div v-if="prepared.status === 'empty'" data-diff-empty>
      <EmptyState
        title="No diff preview"
        description="A bounded text preview will appear when the runtime provides one."
      />
    </div>

    <InlineAlert
      v-else-if="prepared.status === 'refused'"
      tone="warning"
      title="Preview unavailable"
      data-diff-refused
    >
      Binary or control-heavy content is not rendered in the browser.
    </InlineAlert>

    <template v-else>
      <header class="safe-diff-preview__header">
        <span class="safe-diff-preview__state" data-diff-state>
          <ScissorsLineDashed
            v-if="prepared.status === 'truncated'"
            :size="15"
            aria-hidden="true"
          />
          <FileWarning v-else :size="15" aria-hidden="true" />
          {{ prepared.status === 'truncated' ? 'Truncated text preview' : 'Text preview' }}
        </span>
        <AppBadge :tone="prepared.status === 'truncated' ? 'warning' : 'neutral'" mono>
          {{ prepared.byteCount }} bytes
        </AppBadge>
      </header>

      <p
        v-if="prepared.status === 'truncated'"
        class="safe-diff-preview__metadata"
        data-diff-metadata
      >
        Showing {{ prepared.lineCount }} of {{ prepared.originalLineCount }} lines and
        {{ prepared.byteCount }} of {{ prepared.originalByteCount }} bytes.
      </p>

      <pre class="safe-diff-preview__text" data-diff-text>{{ prepared.text }}</pre>
    </template>
  </section>
</template>

<style scoped>
.safe-diff-preview {
  display: flex;
  min-block-size: 100%;
  flex-direction: column;
  gap: var(--space-2);
}

.safe-diff-preview__header,
.safe-diff-preview__state {
  display: flex;
  align-items: center;
}

.safe-diff-preview__header {
  min-block-size: 2rem;
  justify-content: space-between;
  gap: var(--space-2);
}

.safe-diff-preview__state {
  gap: var(--space-2);
  color: var(--color-text-secondary);
  font-size: var(--text-sm);
  font-weight: var(--weight-medium);
}

.safe-diff-preview__metadata {
  margin: 0;
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
}

.safe-diff-preview__text {
  min-block-size: 0;
  flex: 1;
  overflow: auto;
  margin: 0;
  padding: var(--space-3);
  border: 1px solid var(--color-border-base);
  border-radius: var(--radius-sm);
  background: var(--color-bg-base);
  color: var(--color-text-primary);
  font-family: var(--font-mono);
  font-size: var(--text-xs);
  line-height: 1.6;
  tab-size: 2;
  white-space: pre;
}
</style>
