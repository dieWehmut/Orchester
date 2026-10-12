<script setup lang="ts">
import { computed } from 'vue'
import { MarkdownText } from '@orchester/design'
import { ChevronRight, Terminal } from '@lucide/vue'
import type { ActivityDetail } from '../../../api/agent-activity'
import { useI18n } from '../../../i18n'
import MessageActions from '../../../components/run/MessageActions.vue'
import { conversationItems, elapsedLabel, visibleAnswer } from '../conversation'

const props = defineProps<{ detail: ActivityDetail }>()
const { t } = useI18n()
const items = computed(() => conversationItems(props.detail.entries, props.detail.turns))
const activeWork = computed(() => props.detail.state === 'active' && items.value.at(-1)?.kind === 'work' ? items.value.at(-1)?.id : null)
</script>

<template>
  <div class="agent-conversation" :data-conversation-provider="detail.provider">
    <template v-for="item in items" :key="item.id">
      <article v-if="item.kind === 'user'" class="agent-conversation__user" data-activity-entry="user" :aria-label="t('activity.kinds.user')">
        <p>{{ item.entry.text }}</p>
      </article>
      <section v-else-if="item.kind === 'work'" class="agent-conversation__work" :data-work-id="item.id" :data-working="activeWork === item.id">
        <details class="agent-conversation__process" data-work-disclosure>
          <summary>
            <span>{{ activeWork === item.id ? t('activity.working') : item.duration === undefined ? t('activity.work') : t('activity.workedFor', { duration: elapsedLabel(item.duration) }) }}</span>
            <ChevronRight :size="16" aria-hidden="true" />
          </summary>
          <div class="agent-conversation__steps">
            <article v-for="entry in item.entries" :key="entry.id" :data-activity-entry="entry.kind">
              <MarkdownText v-if="entry.kind === 'assistant'" :text="visibleAnswer(entry.text)" :external-label="t('transcript.externalLink')" />
              <details v-else class="agent-conversation__tool" data-tool-disclosure>
                <summary><Terminal :size="14" aria-hidden="true" /><span>{{ entry.tool || t(`activity.kinds.${entry.kind}`) }}</span><ChevronRight :size="14" aria-hidden="true" /></summary>
                <pre>{{ entry.text }}</pre>
              </details>
            </article>
          </div>
        </details>
        <div v-if="activeWork === item.id" class="agent-conversation__progress" data-work-preview><MarkdownText :text="visibleAnswer([...item.entries].reverse().find(entry => entry.kind === 'assistant')?.text || item.entries.at(-1)?.tool || t('activity.waitingContent'))" :external-label="t('transcript.externalLink')" /></div>
      </section>
      <article v-else-if="item.kind === 'assistant'" class="agent-conversation__answer" data-activity-entry="assistant" :aria-label="t('activity.kinds.assistant')">
        <MarkdownText :text="visibleAnswer(item.entry.text)" :external-label="t('transcript.externalLink')" />
        <MessageActions :text="item.entry.text" :label="t('transcript.copyMessage')" :copied-label="t('transcript.copied')" />
      </article>
      <p v-else class="agent-conversation__status" data-activity-entry="status">{{ item.entry.text }}</p>
    </template>
  </div>
</template>

<style scoped>
.agent-conversation { --conversation-code-surface: var(--color-bg-element); inline-size: min(100%, 48rem); margin-inline: auto; padding-block: 12px 40px; color: var(--color-text-primary); font-size: var(--text-md); line-height: 1.75; }
[data-conversation-provider='codex'] { --conversation-code-surface: #303030; }
:root[data-theme='light'] [data-conversation-provider='codex'] { --conversation-code-surface: var(--color-bg-element); }
.agent-conversation__user { display: flex; justify-content: flex-end; margin-block: 0 52px; }
.agent-conversation__user:not(:first-child) { margin-block-start: 52px; }
.agent-conversation__user p { max-inline-size: min(85%, 38rem); margin: 0; padding: 12px 20px; border-radius: 22px; background: var(--color-bg-element); white-space: pre-wrap; overflow-wrap: anywhere; }
[data-conversation-provider='codex'] .agent-conversation__user p { background: #843d63; color: #fff; }
.agent-conversation__work { margin-block: 0 22px; padding-block-end: 8px; border-block-end: 1px solid var(--color-border-subtle); }
.agent-conversation__process > summary { display: flex; align-items: center; gap: 8px; inline-size: fit-content; min-block-size: 32px; color: var(--color-text-tertiary); font-size: var(--text-sm); cursor: pointer; list-style: none; }
summary::-webkit-details-marker { display: none; }
summary > svg:last-child { flex-shrink: 0; transition: transform var(--transition-fast); }
details[open] > summary > svg:last-child { transform: rotate(90deg); }
.agent-conversation__steps { display: grid; gap: 16px; padding: 16px 0 12px; color: var(--color-text-secondary); font-size: var(--text-sm); }
.agent-conversation__steps > article { min-inline-size: 0; }
.agent-conversation__tool { border-inline-start: 2px solid var(--color-border-base); padding-inline-start: 12px; }
.agent-conversation__tool summary { display: flex; align-items: center; gap: 8px; min-block-size: 32px; cursor: pointer; list-style: none; }
.agent-conversation__tool pre { max-block-size: 24rem; overflow: auto; margin: 8px 0; padding: 12px; border-radius: 8px; background: var(--color-bg-element); font: 12px/1.6 var(--font-mono); white-space: pre-wrap; overflow-wrap: anywhere; }
.agent-conversation__progress { margin: 4px 0 8px; color: var(--color-text-secondary); font-size: var(--text-sm); display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
.agent-conversation__answer { min-inline-size: 0; margin-block-end: 24px; }
.agent-conversation__answer :deep(.markdown) { gap: 16px; }
.agent-conversation__answer :deep(.markdown__list) { gap: 4px; padding-inline-start: 30px; }
.agent-conversation :deep(.markdown__list[data-list-ordered='false']) { list-style-type: disc; }
.agent-conversation :deep(.markdown__list[data-list-ordered='true']) { list-style-type: decimal; }
.agent-conversation__answer :deep(.markdown-spans__code) { border: 0; border-radius: 7px; padding: 2px 7px; background: var(--conversation-code-surface); font-size: .88em; font-weight: 600; }
.agent-conversation__answer :deep(.markdown__code) { border: 0; border-radius: 12px; padding: 16px; background: var(--conversation-code-surface); font-size: 13px; }
.agent-conversation__answer :deep(.markdown__table) { font-size: 14px; }
.agent-conversation__answer :deep(.message-actions) { margin-block-start: 14px; }
.agent-conversation__status { color: var(--color-text-secondary); font-size: var(--text-sm); }
summary:focus-visible { outline: 2px solid var(--color-border-focus); outline-offset: 3px; border-radius: 4px; }
[data-conversation-provider='claude'] .agent-conversation__tool { border-inline-start-color: #c38c73; }
[data-conversation-provider='opencode'] .agent-conversation__user p { border-radius: 8px; border-inline-start: 2px solid var(--color-accent); }
@media (max-width: 640px) { .agent-conversation { font-size: 15px; } .agent-conversation__user p { max-inline-size: 92%; } .agent-conversation__user { margin-block-end: 36px; } }
</style>
