<script setup lang="ts">
import type { ApprovalView } from '@orchester/ereignis'
import { AppBadge, AppButton, EmptyState, InlineAlert } from '@orchester/design'
import { computed } from 'vue'

import { useI18n } from '../../i18n'

/**
 * The Approvals queue, section 4.7 of the design spec.
 *
 * Each card carries the risk summary, the redacted action and the exact scope,
 * and the three scoped choices the spec names. The queue emits a decision
 * intent rather than deciding: the row version a decision is made against is
 * what makes a stale entry detectable, so the intent carries it and the caller
 * sends it back to the runtime.
 *
 * A superseded entry is its own state, never an error. Nothing failed - the
 * row the reader was looking at has simply moved on - so it renders as news
 * with the choices withdrawn, which is what "this decision belongs to a newer
 * version" looks like.
 */

const props = withDefaults(
  defineProps<{
    approvals: readonly ApprovalView[]
    /**
     * Approvals the runtime refused as stale, keyed by approval id.
     *
     * A refusal is not an event in the run's stream, so the projection cannot
     * know about it; the caller carries it back so the row the reader just
     * decided on shows the superseded state rather than snapping back to
     * pending as though nothing had happened.
     */
    superseded?: readonly string[]
  }>(),
  { superseded: () => [] },
)

const { t } = useI18n()

const emit = defineEmits<{
  decide: [decision: { approvalId: string; rowVersion: number; decision: 'approved' | 'denied' }]
}>()

/** The three choices the spec names, in the order they are offered. */
const CHOICES = [
  { id: 'allow-once', decision: 'approved' },
  { id: 'allow-for-run', decision: 'approved' },
  { id: 'deny', decision: 'denied' },
] as const

function choiceLabel(id: (typeof CHOICES)[number]['id']): string {
  switch (id) {
    case 'allow-once':
      return t('inspector.allowOnce')
    case 'allow-for-run':
      return t('inspector.allowForRun')
    case 'deny':
      return t('inspector.deny')
  }
}

/** The state a row renders in: its own, or superseded when refused. */
function stateOf(approval: ApprovalView): ApprovalView['state'] | 'stale' {
  return props.superseded.includes(approval.approvalId) ? 'stale' : approval.state
}

/** Whether this entry still has a decision to take. */
function isOpen(approval: ApprovalView): boolean {
  return stateOf(approval) === 'pending'
}

function riskTone(risk: string): 'warning' | 'error' | 'neutral' {
  const normalised = risk.trim().toLowerCase()
  if (normalised === 'high' || normalised === 'critical') return 'error'
  if (normalised === 'medium' || normalised === 'moderate') return 'warning'
  return 'neutral'
}

function decide(approval: ApprovalView, decision: 'approved' | 'denied'): void {
  emit('decide', {
    approvalId: approval.approvalId,
    rowVersion: approval.rowVersion,
    decision,
  })
}

/** The queue in the order the run asked for it, oldest request first. */
const ordered = computed(() =>
  [...props.approvals].sort(
    (left, right) => (left.requestedSequence ?? 0) - (right.requestedSequence ?? 0),
  ),
)
</script>

<template>
  <section class="approvals-queue" :aria-label="t('inspector.approvalsTitle')" data-approvals-queue>
    <div v-if="ordered.length === 0" data-approvals-empty>
      <EmptyState
        :title="t('inspector.nothingToDecide')"
        :description="t('inspector.nothingToDecideDescription')"
      />
    </div>

    <ul v-else class="approvals-queue__list" :aria-label="t('inspector.queueLabel')">
      <li
        v-for="approval in ordered"
        :key="approval.key"
        class="approvals-queue__card"
        :data-approval-id="approval.approvalId"
        :data-approval-state="stateOf(approval)"
      >
        <header class="approvals-queue__head">
          <AppBadge :tone="riskTone(approval.risk)">
            {{ t('inspector.risk') }}: {{ approval.risk }}
          </AppBadge>
          <span class="approvals-queue__sequence" aria-hidden="true">#{{ approval.requestedSequence }}</span>
        </header>

        <p class="approvals-queue__action">{{ approval.action }}</p>
        <p class="approvals-queue__reason">{{ approval.reason }}</p>
        <p class="approvals-queue__scope" data-approval-scope>
          {{ t('inspector.scope') }}: {{ approval.runId }}
        </p>

        <InlineAlert
          v-if="stateOf(approval) === 'stale'"
          tone="info"
          :title="t('inspector.superseded')"
          data-approval-superseded
        >
          {{ t('inspector.supersededDescription') }}
        </InlineAlert>

        <p v-else-if="!isOpen(approval)" class="approvals-queue__decided" data-approval-decided>
          {{ t('inspector.decided') }}: {{ approval.state }}
        </p>

        <div v-else class="approvals-queue__choices">
          <AppButton
            v-for="choice in CHOICES"
            :key="choice.id"
            variant="ghost"
            size="sm"
            :data-approval-choice="choice.id"
            @click="decide(approval, choice.decision)"
          >
            {{ choiceLabel(choice.id) }}
          </AppButton>
        </div>
      </li>
    </ul>
  </section>
</template>

<style scoped>
.approvals-queue {
  display: flex;
  min-block-size: 100%;
  flex-direction: column;
  gap: var(--space-2);
}

.approvals-queue__list {
  display: grid;
  gap: var(--space-3);
  margin: 0;
  padding: 0;
  list-style: none;
}

.approvals-queue__card {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-3);
  border: 1px solid var(--color-border-base);
  border-radius: var(--radius-sm);
  background: var(--color-bg-surface);
}

.approvals-queue__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
}

.approvals-queue__sequence {
  color: var(--color-text-tertiary);
  font-family: var(--font-mono);
  font-size: var(--text-xs);
}

.approvals-queue__action,
.approvals-queue__reason,
.approvals-queue__scope,
.approvals-queue__decided {
  margin: 0;
  font-size: var(--text-sm);
}

.approvals-queue__action {
  color: var(--color-text-primary);
  font-family: var(--font-mono);
  overflow-wrap: anywhere;
}

.approvals-queue__reason,
.approvals-queue__scope,
.approvals-queue__decided {
  color: var(--color-text-secondary);
}

.approvals-queue__choices {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
}
</style>
