<script setup lang="ts">
import { computed } from 'vue'
import { MarkdownText } from '@orchester/design'
import { CircleAlert } from '@lucide/vue'
import type { ActivityDetail } from '../../../api/agent-activity'
import { useI18n } from '../../../i18n'
import MessageActions from '../../../components/run/MessageActions.vue'
import { conversationItems, visibleAnswer } from '../conversation'
import AgentWorkBlock from './AgentWorkBlock.vue'

const props = defineProps<{ detail: ActivityDetail }>()
const { t, locale } = useI18n()
const items = computed(() => conversationItems(props.detail.entries, props.detail.turns))
const activeWork = computed(() => props.detail.state === 'active' && items.value.at(-1)?.kind === 'work' ? items.value.at(-1)?.id : null)
const dateLabels = computed(() => {
  const labels = new Map<string, string>()
  let previous = 0
  for (const item of items.value) {
    if (item.kind !== 'user' || item.entry.timestamp === undefined) continue
    const at = item.entry.timestamp
    if (!previous || at - previous > 30 * 60_000) labels.set(item.id, new Intl.DateTimeFormat(locale.value, { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' }).format(new Date(at)))
    previous = at
  }
  return labels
})
</script>

<template>
  <div class="agent-conversation" :data-conversation-provider="detail.provider">
    <template v-for="item in items" :key="item.id">
      <time v-if="dateLabels.has(item.id)" class="agent-conversation__date" data-conversation-date>{{ dateLabels.get(item.id) }}</time>
      <article v-if="item.kind === 'user'" class="agent-conversation__user" data-activity-entry="user" :aria-label="t('activity.kinds.user')"><p>{{ item.entry.text }}</p></article>
      <AgentWorkBlock v-else-if="item.kind === 'work'" :item="item" :active="activeWork === item.id" />
      <article v-else-if="item.kind === 'assistant'" class="agent-conversation__answer" data-activity-entry="assistant" :aria-label="t('activity.kinds.assistant')">
        <MarkdownText :text="visibleAnswer(item.entry.text)" :external-label="t('transcript.externalLink')" :file-label="t('transcript.copyFilePath')" />
        <MessageActions :text="visibleAnswer(item.entry.text)" :label="t('transcript.copyMessage')" :copied-label="t('transcript.copied')" />
      </article>
      <p v-else class="agent-conversation__status" data-activity-entry="status"><CircleAlert :size="14" />{{ item.entry.text }}</p>
    </template>
  </div>
</template>

<style scoped>
.agent-conversation { --conversation-code-surface: #303030; inline-size: min(100%, 48rem); margin-inline: auto; padding-block: 12px 32px; color: var(--color-text-primary); font-size: 16px; line-height: 1.75; }
:global([data-theme='light'] .agent-conversation) { --conversation-code-surface: var(--color-bg-element); }
.agent-conversation__date { display: block; margin: 16px 0 28px; text-align: center; font-size: 12px; color: var(--color-text-tertiary); }
.agent-conversation__user { display: flex; justify-content: flex-end; margin-block: 0 44px; }
.agent-conversation__user:not(:first-child) { margin-block-start: 36px; }
.agent-conversation__date + .agent-conversation__user { margin-block-start: 0; }
.agent-conversation__user p { max-inline-size: min(85%, 38rem); margin: 0; padding: 10px 18px; border-radius: 20px; background: #843d63; color: #fff; white-space: pre-wrap; overflow-wrap: anywhere; }
.agent-conversation__answer { min-inline-size: 0; margin-block-end: 32px; }
.agent-conversation :deep(.markdown) { gap: 16px; }
.agent-conversation :deep(.markdown__list) { gap: 5px; padding-inline-start: 28px; }
.agent-conversation :deep(.markdown__list[data-list-ordered='false']) { list-style-type: disc; }
.agent-conversation :deep(.markdown__list[data-list-ordered='true']) { list-style-type: decimal; }
.agent-conversation :deep(.markdown-spans__code) { border: 0; border-radius: 6px; padding: 2px 6px; background: var(--conversation-code-surface); font-size: .88em; font-weight: 600; }
.agent-conversation :deep(.markdown__code) { border: 0; border-radius: 10px; padding: 16px; background: var(--conversation-code-surface); font-size: 13px; }
.agent-conversation :deep(.markdown__table) { font-size: 14px; }
.agent-conversation :deep(.markdown__table th), .agent-conversation :deep(.markdown__table td) { border: 0; border-block-end: 1px solid var(--color-border-subtle); padding: 12px 16px; }
.agent-conversation :deep(.markdown__table th) { background: transparent; }
.agent-conversation__answer :deep(.message-actions) { margin-block-start: 12px; }
.agent-conversation__status { display: flex; align-items: center; justify-content: center; gap: 8px; padding-block: 14px; color: var(--color-text-secondary); font-size: 12px; border-block: 1px solid var(--color-border-subtle); }
.agent-conversation__status svg { flex-shrink: 0; }
@media (max-width: 640px) { .agent-conversation { font-size: 15px; } .agent-conversation__user p { max-inline-size: 92%; } }
</style>