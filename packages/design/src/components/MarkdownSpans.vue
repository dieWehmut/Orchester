<script setup lang="ts">
/**
 * The spans of one line of an answer.
 *
 * A link is drawn with the browser's own safety attributes and, when the caller
 * supplies the words, with what it does said out loud: an answer can link
 * anywhere, and a reader who is not looking at the pointer still deserves to
 * know the link leaves the page.
 */
import type { MarkdownInline } from '../markdown'
import { ref } from 'vue'
import { Check, FileText } from '@lucide/vue'
import VisuallyHidden from './VisuallyHidden.vue'

withDefaults(
  defineProps<{
    spans: readonly MarkdownInline[]
    /** Announced inside every link. Empty means the surface has no words for it. */
    externalLabel?: string
    fileLabel?: string
  }>(),
  { externalLabel: '', fileLabel: 'Copy file path' },
)
const copied = ref<string | null>(null)
const failed = ref<string | null>(null)
async function copyPath(path: string): Promise<void> {
  try { await navigator.clipboard.writeText(path); copied.value = path; failed.value = null }
  catch { failed.value = path }
}
</script>

<template>
  <template v-for="(span, index) in spans" :key="index">
    <code v-if="span.kind === 'code'" class="markdown-spans__code" data-markdown-inline-code>{{
      span.text
    }}</code>
    <strong v-else-if="span.kind === 'strong'" class="markdown-spans__strong" data-markdown-strong>{{
      span.text
    }}</strong>
    <template v-else-if="span.kind === 'file'">
      <button type="button" class="markdown-spans__file" data-markdown-file :title="`${fileLabel}: ${span.path}`" :aria-label="`${span.text} — ${fileLabel}`" @click="copyPath(span.path)"><Check v-if="copied === span.path" :size="15" aria-hidden="true" /><FileText v-else :size="15" aria-hidden="true" />{{ span.text }}</button>
      <code v-if="failed === span.path" class="markdown-spans__path" role="status">{{ span.path }}</code>
    </template>
    <a
      v-else-if="span.kind === 'link'"
      class="markdown-spans__link"
      data-markdown-link
      :href="span.href"
      target="_blank"
      rel="noopener noreferrer"
      >{{ span.text }}<VisuallyHidden v-if="externalLabel">{{ externalLabel }}</VisuallyHidden></a
    >
    <template v-else>{{ span.text }}</template>
  </template>
</template>

<style scoped>
.markdown-spans__code {
  padding: 0.1em 0.35em;
  border: 1px solid var(--color-border-base);
  border-radius: var(--radius-xs);
  background: var(--color-bg-element);
  font-family: var(--font-mono);
  font-size: 0.94em;
}

.markdown-spans__strong {
  font-weight: var(--weight-semibold);
}

.markdown-spans__link {
  color: var(--color-accent);
  text-decoration: underline;
  text-underline-offset: 2px;
}

.markdown-spans__link:focus-visible {
  outline: 2px solid var(--color-border-focus);
  outline-offset: 2px;
}

.markdown-spans__file { display: inline; border: 0; padding: 0; background: transparent; color: var(--color-accent); font: inherit; font-weight: 500; text-align: start; cursor: pointer; overflow-wrap: anywhere; }
.markdown-spans__file svg { display: inline; margin-inline-end: 5px; vertical-align: -.15em; }
.markdown-spans__file:hover { text-decoration: underline; text-underline-offset: 3px; }
.markdown-spans__file:focus-visible { outline: 2px solid var(--color-border-focus); outline-offset: 2px; }
.markdown-spans__path { display: inline; margin-inline-start: 6px; font-size: .85em; user-select: all; overflow-wrap: anywhere; }
</style>
