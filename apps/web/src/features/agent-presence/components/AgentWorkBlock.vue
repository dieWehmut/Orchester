<script setup lang="ts">
import { computed, ref } from 'vue'
import { MarkdownText } from '@orchester/design'
import { ChevronRight, FilePenLine, FileText, Images, Plug, Search, Terminal, Wrench } from '@lucide/vue'
import { elapsedLabel, toolPresentation, visibleAnswer, workSteps, type ConversationItem } from '../conversation'
import { useI18n } from '../../../i18n'
const props = defineProps<{ item: Extract<ConversationItem, { kind: 'work' }>; active: boolean }>()
const { t } = useI18n()
const open = ref(props.active)
const steps = computed(() => workSteps(props.item.entries))
const icons = { command: Terminal, edit: FilePenLine, read: FileText, search: Search, image: Images, integration: Plug, tool: Wrench }
</script>

<template>
  <section class="agent-work" :data-work-id="item.id" :data-working="active">
    <details class="agent-work__disclosure" data-work-disclosure :open="open" @toggle="open = ($event.currentTarget as HTMLDetailsElement).open">
      <summary><span>{{ active ? t('activity.working') : item.duration === undefined ? t('activity.work') : t('activity.workedFor', { duration: elapsedLabel(item.duration) }) }}</span><ChevronRight :size="14" aria-hidden="true" /></summary>
      <div class="agent-work__steps">
        <article v-for="step in steps" :key="step.id" :data-activity-entry="step.entry.kind">
          <MarkdownText v-if="step.entry.kind === 'assistant'" :text="visibleAnswer(step.entry.text)" :external-label="t('transcript.externalLink')" :file-label="t('transcript.copyFilePath')" />
          <details v-else class="agent-work__tool" data-tool-disclosure>
            <summary :title="step.entry.tool || ''"><component :is="icons[toolPresentation(step.entry)]" :size="14" aria-hidden="true" /><span>{{ t(`activity.actions.${toolPresentation(step.entry)}`) }}</span><ChevronRight :size="13" aria-hidden="true" /></summary>
            <div class="agent-work__output"><p v-if="step.entry.tool" class="agent-work__name">{{ step.entry.tool }}</p><pre>{{ step.entry.text }}</pre><pre v-for="result in step.results" :key="result.id" data-tool-result>{{ result.text }}</pre></div>
          </details>
        </article>
      </div>
    </details>
    <p v-if="active && !open" class="agent-work__preview" data-work-preview>{{ visibleAnswer([...item.entries].reverse().find(entry => entry.kind === 'assistant')?.text || t('activity.waitingContent')) }}</p>
  </section>
</template>

<style scoped>
.agent-work { margin-block: 0 24px; border-block-end: 1px solid var(--color-border-subtle); }
.agent-work__disclosure > summary { display: flex; align-items: center; gap: 6px; min-block-size: 32px; padding-block-end: 6px; color: var(--color-text-tertiary); font-size: 13px; cursor: pointer; list-style: none; }
summary::-webkit-details-marker { display: none; }
summary > svg:last-child { flex-shrink: 0; transition: transform var(--transition-fast); }
details[open] > summary > svg:last-child { transform: rotate(90deg); }
.agent-work__steps { display: grid; gap: 16px; padding: 12px 0 20px; font-size: 14px; line-height: 1.7; }
.agent-work__steps > article { min-inline-size: 0; }
.agent-work__tool summary { display: flex; align-items: center; gap: 7px; min-block-size: 26px; color: var(--color-text-tertiary); cursor: pointer; list-style: none; font-size: 12px; }
.agent-work__tool summary svg:last-child { opacity: 0; }
.agent-work__tool summary:hover svg:last-child, .agent-work__tool[open] summary svg:last-child { opacity: 1; }
.agent-work__output { padding-inline-start: 20px; }
.agent-work__output pre { max-block-size: 24rem; overflow: auto; margin: 8px 0; padding: 12px; border-radius: 8px; background: var(--color-bg-element); font: 12px/1.6 var(--font-mono); white-space: pre-wrap; overflow-wrap: anywhere; }
.agent-work__name { margin: 6px 0; color: var(--color-text-tertiary); font-size: 11px; }
.agent-work__preview { margin: 4px 0 12px; color: var(--color-text-secondary); font-size: 13px; display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
summary:focus-visible { outline: 2px solid var(--color-border-focus); outline-offset: 2px; border-radius: 4px; }
</style>
