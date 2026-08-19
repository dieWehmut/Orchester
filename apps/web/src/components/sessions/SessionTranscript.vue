<script setup lang="ts">
import type { SessionDetailDto } from '@orchester/protokoll'
import { EmptyState, InlineAlert, SkeletonBlock } from '@orchester/design'

import { useI18n } from '../../i18n'
import type { DetailStatus } from '../../stores/sessions'

defineProps<{
  status: DetailStatus
  session: SessionDetailDto | null
  error: { message: string } | null
}>()

const { t } = useI18n()
</script>

<template>
  <div class="session-transcript">
    <div v-if="status === 'loading'" class="session-transcript__loading" role="status">
      <span>{{ t('transcript.loading') }}</span>
      <SkeletonBlock :lines="5" height="1.2rem" />
    </div>

    <InlineAlert v-else-if="status === 'error'" tone="error" :title="t('transcript.loadError')">
      {{ error?.message }}
    </InlineAlert>

    <article v-else-if="session" class="session-transcript__content" data-session-transcript>
      <header class="session-transcript__header">
        <div>
          <p>{{ session.agent }}<template v-if="session.model"> / {{ session.model }}</template></p>
          <h1>{{ session.title }}</h1>
        </div>
        <span class="session-transcript__outcome" :data-outcome="session.outcome">
          {{ session.outcome }}
        </span>
      </header>

      <section class="session-transcript__turn session-transcript__turn--user">
        <h2>{{ t('transcript.prompt') }}</h2>
        <p>{{ session.prompt }}</p>
      </section>

      <section class="session-transcript__turn session-transcript__turn--assistant">
        <h2>{{ t('transcript.result') }}</h2>
        <p>{{ session.final_text }}</p>
      </section>

      <footer class="session-transcript__usage" :aria-label="t('transcript.usage')">
        <span>{{ t('transcript.inputTokens') }} {{ session.usage.input_tokens }}</span>
        <span>{{ t('transcript.outputTokens') }} {{ session.usage.output_tokens }}</span>
        <span>{{ t('transcript.reasoningTokens') }} {{ session.usage.reasoning_output_tokens }}</span>
        <span>{{ t('transcript.cachedTokens') }} {{ session.usage.cached_input_tokens }}</span>
      </footer>
    </article>

    <EmptyState
      v-else
      :title="t('transcript.emptyTitle')"
      :description="t('transcript.emptyDescription')"
    />
  </div>
</template>

<style scoped>
.session-transcript {
  min-block-size: 100%;
  padding: var(--space-6);
}

.session-transcript__loading,
.session-transcript__content {
  inline-size: min(100%, 52rem);
  margin-inline: auto;
}

.session-transcript__loading {
  display: grid;
  gap: var(--space-4);
  padding-block: var(--space-8);
  color: var(--color-text-secondary);
}

.session-transcript__content {
  display: grid;
  gap: var(--space-5);
}

.session-transcript__header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--space-4);
  padding-block-end: var(--space-4);
  border-block-end: 1px solid var(--color-border-base);
}

.session-transcript__header p,
.session-transcript__header h1,
.session-transcript__turn h2,
.session-transcript__turn p {
  margin: 0;
}

.session-transcript__header p {
  color: var(--color-text-tertiary);
  font-family: var(--font-mono);
  font-size: var(--text-xs);
}

.session-transcript__header h1 {
  margin-block-start: var(--space-1);
  font-size: var(--text-lg);
  letter-spacing: 0;
}

.session-transcript__outcome {
  flex: 0 0 auto;
  padding: var(--space-1) var(--space-2);
  border: 1px solid var(--color-border-base);
  border-radius: var(--radius-xs);
  color: var(--color-text-secondary);
  font-family: var(--font-mono);
  font-size: var(--text-xs);
  text-transform: uppercase;
}

.session-transcript__outcome[data-outcome='success'] {
  border-color: var(--color-status-success);
  color: var(--color-status-success);
}

.session-transcript__outcome[data-outcome='failed'] {
  border-color: var(--color-status-error);
  color: var(--color-status-error);
}

.session-transcript__turn {
  display: grid;
  gap: var(--space-2);
  padding: var(--space-4);
  border-inline-start: 2px solid var(--color-border-strong);
  background: var(--color-bg-surface);
}

.session-transcript__turn--assistant {
  border-inline-start-color: var(--color-accent);
}

.session-transcript__turn h2 {
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
  letter-spacing: 0.04em;
  text-transform: uppercase;
}

.session-transcript__turn p {
  overflow-wrap: anywhere;
  white-space: pre-wrap;
}

.session-transcript__usage {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-3);
  color: var(--color-text-tertiary);
  font-family: var(--font-mono);
  font-size: var(--text-xs);
}

@media (max-width: 640px) {
  .session-transcript {
    padding: var(--space-4);
  }

  .session-transcript__header {
    align-items: stretch;
    flex-direction: column;
  }

  .session-transcript__outcome {
    align-self: flex-start;
  }
}
</style>
