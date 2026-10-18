<script setup lang="ts">
import { computed, ref } from 'vue'
import { MarkdownText } from '@orchester/design'
import { ChevronRight, FilePenLine, FileText, Images, LoaderCircle, Plug, Search, Terminal, Wrench } from '@lucide/vue'
import { elapsedLabel, toolIntegration, toolPresentation, visibleAnswer, workSteps, type ConversationItem } from '../conversation'
import { useI18n } from '../../../i18n'
const props = defineProps<{ item: Extract<ConversationItem, { kind: 'work' }>; active: boolean; latest: boolean; now: number }>()
const { t } = useI18n()
// The latest round reads as a conversation. Refreshes must never overwrite a
// reader's disclosure choice, including when a running round completes.
const open = ref(props.active || props.latest)
const steps = computed(() => workSteps(props.item.entries))
const duration = computed(() => props.active && props.item.startedAt !== undefined
  ? Math.max(0, props.now - props.item.startedAt) : props.item.duration)
const label = computed(() => duration.value === undefined ? t(props.active ? 'activity.working' : 'activity.work')
  : t(props.active ? 'activity.workingFor' : 'activity.workedFor', { duration: elapsedLabel(duration.value) }))
const icons = { command: Terminal, edit: FilePenLine, read: FileText, search: Search, image: Images, integration: Plug, tool: Wrench }
</script>

<template>
  <section class="agent-work" :data-work-id="item.id" :data-working="active">
    <details class="agent-work__disclosure" data-work-disclosure :open="open" @toggle="open = ($event.currentTarget as HTMLDetailsElement).open">
      <summary><span>{{ label }}</span><ChevronRight :size="14" aria-hidden="true" /></summary>
      <div class="agent-work__steps">
        <article v-for="step in steps" :key="step.id" :data-activity-entry="step.entry.kind">
          <MarkdownText v-if="step.entry.kind === 'assistant'" :text="visibleAnswer(step.entry.text)" :external-label="t('transcript.externalLink')" :file-label="t('transcript.copyFilePath')" />
          <details v-else class="agent-work__tool" data-tool-disclosure :data-tool-pending="active && step.entry.kind === 'tool' && !step.results.length" :data-integration="toolIntegration(step.entry) || undefined">
            <summary :title="step.entry.tool || ''">
              <LoaderCircle v-if="active && step.entry.kind === 'tool' && !step.results.length" class="agent-work__spinner" :size="16" :aria-label="t('activity.working')" />
              <component v-else :is="icons[toolPresentation(step.entry)]" :size="16" aria-hidden="true" />
              <span>{{ toolIntegration(step.entry) ? t('activity.integrationUsed', { name: toolIntegration(step.entry)! }) : t(`activity.actions.${toolPresentation(step.entry)}`) }}</span>
              <ChevronRight :size="13" aria-hidden="true" />
            </summary>
            <div class="agent-work__output"><p v-if="step.entry.tool" class="agent-work__name">{{ step.entry.tool }}</p><pre>{{ step.entry.text }}</pre><pre v-for="result in step.results" :key="result.id" data-tool-result>{{ result.text }}</pre></div>
          </details>
        </article>
      </div>
    </details>
    <p v-if="active && !open" class="agent-work__preview" data-work-preview>{{ visibleAnswer([...item.entries].reverse().find(entry => entry.kind === 'assistant')?.text || t('activity.waitingContent')) }}</p>
  </section>
</template>

<style scoped>
.agent-work { margin-block: 0 28px; }
.agent-work__disclosure > summary { display: flex; align-items: center; gap: 6px; min-block-size: 32px; padding-block-end: 8px; border-block-end: 1px solid var(--color-border-subtle); color: var(--color-text-tertiary); font-size: 14px; cursor: pointer; list-style: none; }
summary::-webkit-details-marker { display: none; }
summary > svg:last-child { flex-shrink: 0; transition: transform var(--transition-fast); }
details[open] > summary > svg:last-child { transform: rotate(90deg); }
.agent-work__steps { display: grid; gap: 24px; padding: 20px 0 0; font-size: inherit; line-height: inherit; }
.agent-work__steps > article { min-inline-size: 0; }
.agent-work__tool summary { display: flex; align-items: center; gap: 8px; min-block-size: 28px; color: var(--color-text-tertiary); cursor: pointer; list-style: none; font-size: 14px; }
.agent-work__tool summary > svg { flex-shrink: 0; }
.agent-work__tool[data-integration] summary > svg:first-child { color: #17a994; }
.agent-work__spinner { animation: activity-spin 1.2s linear infinite; }
@keyframes activity-spin { to { transform: rotate(360deg); } }
@media (prefers-reduced-motion: reduce) { .agent-work__spinner { animation: none; } }
.agent-work__tool summary svg:last-child { opacity: 0; }
.agent-work__tool summary:hover svg:last-child, .agent-work__tool[open] summary svg:last-child { opacity: 1; }
.agent-work__output { padding-inline-start: 20px; }
.agent-work__output pre { max-block-size: 24rem; overflow: auto; margin: 8px 0; padding: 12px; border-radius: 8px; background: var(--color-bg-element); font: 12px/1.6 var(--font-mono); white-space: pre-wrap; overflow-wrap: anywhere; }
.agent-work__name { margin: 6px 0; color: var(--color-text-tertiary); font-size: 11px; }
.agent-work__preview { margin: 4px 0 12px; color: var(--color-text-secondary); font-size: 13px; display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
summary:focus-visible { outline: 2px solid var(--color-border-focus); outline-offset: 2px; border-radius: 4px; }
</style>
