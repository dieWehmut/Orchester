<script setup lang="ts">
/**
 * An answer, rendered from the markdown it was written in.
 *
 * The blocks come from `parseMarkdown`, which produces tokens rather than HTML,
 * and every one of them is drawn by a Vue element here: there is no `v-html` in
 * this component and there is no string of markup to put in one, so text an
 * agent wrote cannot become part of the page's structure.
 *
 * Every heading is drawn as the same element whatever level it was written at.
 * An answer is not the page, and an `h1` inside a transcript would either
 * outrank the page's own title or skip levels in the outline; the written level
 * is kept on the element for the reader and the styling, not for the outline.
 */
import { computed, onScopeDispose, ref } from 'vue'
import { Copy, Check } from '@lucide/vue'

import { parseMarkdown } from '../markdown'
import { codeTokens } from '../code-tokens'
import MarkdownSpans from './MarkdownSpans.vue'

const props = withDefaults(
  defineProps<{
    text: string
    /** Announced inside every link; the surface owns the words. */
    externalLabel?: string
    fileLabel?: string
    copyLabel?: string
    copiedLabel?: string
  }>(),
  { externalLabel: '', fileLabel: 'Copy file path', copyLabel: 'Copy code', copiedLabel: 'Copied' },
)

const copied = ref<number | null>(null)
let copyTimer: ReturnType<typeof setTimeout> | undefined
async function copyCode(text: string, index: number) {
  try {
    await navigator.clipboard.writeText(text)
    copied.value = index
    clearTimeout(copyTimer)
    copyTimer = setTimeout(() => { copied.value = null }, 2000)
  } catch { /* Keep the selectable code visible when clipboard access is unavailable. */ }
}
onScopeDispose(() => clearTimeout(copyTimer))
const blocks = computed(() => parseMarkdown(props.text))
</script>

<template>
  <div class="markdown" data-markdown>
    <template v-for="(block, index) in blocks" :key="index">
      <h3
        v-if="block.kind === 'heading'"
        class="markdown__heading"
        data-markdown-heading
        :data-heading-level="block.level"
      >
        <MarkdownSpans :spans="block.spans" :external-label="externalLabel" :file-label="fileLabel" />
      </h3>
      <section v-else-if="block.kind === 'code'" class="markdown__code-block">
        <header class="markdown__code-header">
          <span>{{ block.language || 'text' }}</span>
          <button type="button" data-markdown-code-copy :aria-label="copied === index ? copiedLabel : copyLabel" :title="copyLabel" @click="copyCode(block.text, index)">
            <Check v-if="copied === index" :size="15" aria-hidden="true" /><Copy v-else :size="15" aria-hidden="true" />
            <span>{{ copied === index ? copiedLabel : copyLabel }}</span>
          </button>
        </header>
        <pre class="markdown__code" data-markdown-code :data-code-language="block.language ?? ''"><code><span v-for="(token, tokenIndex) in codeTokens(block.text, block.language)" :key="tokenIndex" :class="`code-token--${token.kind}`">{{ token.text }}</span></code></pre>
      </section>
      <blockquote v-else-if="block.kind === 'quote'" class="markdown__quote"><MarkdownSpans :spans="block.spans" :external-label="externalLabel" :file-label="fileLabel" /></blockquote>
      <div
        v-else-if="block.kind === 'table'"
        class="markdown__table-scroll"
        data-markdown-table-scroll
      >
        <table class="markdown__table" data-markdown-table>
          <thead>
            <tr>
              <th
                v-for="(cell, column) in block.headers"
                :key="column"
                :style="{ textAlign: block.align[column] ?? 'start' }"
              >
                <MarkdownSpans :spans="cell" :external-label="externalLabel" :file-label="fileLabel" />
              </th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="(row, rowIndex) in block.rows" :key="rowIndex">
              <td
                v-for="(cell, column) in row"
                :key="column"
                :style="{ textAlign: block.align[column] ?? 'start' }"
              >
                <MarkdownSpans :spans="cell" :external-label="externalLabel" :file-label="fileLabel" />
              </td>
            </tr>
          </tbody>
        </table>
      </div>
      <component
        :is="block.ordered ? 'ol' : 'ul'"
        v-else-if="block.kind === 'list'"
        class="markdown__list"
        data-markdown-list
        :data-list-ordered="String(block.ordered)"
      >
        <li v-for="(item, itemIndex) in block.items" :key="itemIndex">
          <MarkdownSpans :spans="item" :external-label="externalLabel" :file-label="fileLabel" />
        </li>
      </component>
      <p v-else class="markdown__paragraph" data-markdown-paragraph>
        <MarkdownSpans :spans="block.spans" :external-label="externalLabel" :file-label="fileLabel" />
      </p>
    </template>
  </div>
</template>

<style scoped>
.markdown {
  display: grid;
  gap: var(--space-3);
  min-inline-size: 0;
  font-synthesis: weight style;
}

.markdown__paragraph,
.markdown__heading,
.markdown__list {
  margin: 0;
  overflow-wrap: anywhere;
}

/* A table is wider than the prose it sits in more often than not, and a
   transcript that scrolled sideways as a whole would take the conversation with
   it: the table scrolls, the transcript does not. */
.markdown__table-scroll {
  max-inline-size: 100%;
  overflow-x: auto;
}

.markdown__table {
  border-collapse: collapse;
  inline-size: 100%;
  font-size: var(--text-sm);
}

.markdown__table th,
.markdown__table td {
  padding: var(--space-2) var(--space-3);
  border: 1px solid var(--color-border-base);
  text-align: start;
  vertical-align: top;
}

.markdown__table th {
  background: var(--color-bg-element);
  font-weight: var(--weight-medium);
}

.markdown__heading {
  font-size: var(--text-base);
  font-weight: var(--weight-semibold);
  line-height: var(--leading-tight);
}

/* The written level still reads as a level, even though the outline does not
   carry it: this is a difference in type, not in structure. */
.markdown__heading[data-heading-level='1'] {
  font-size: 1.65em;
}

.markdown__heading[data-heading-level='2'] {
  font-size: 1.4em;
}

.markdown__heading[data-heading-level='3'] {
  font-size: 1.2em;
}

.markdown__heading[data-heading-level='4'],
.markdown__heading[data-heading-level='5'],
.markdown__heading[data-heading-level='6'] {
  font-size: 1em;
  color: var(--color-text-secondary);
}

.markdown__list {
  display: grid;
  gap: var(--space-1);
  padding-inline-start: var(--space-5);
  list-style: disc outside;
}

.markdown__list[data-list-ordered='true'] { list-style-type: decimal; }

.markdown__code-block { min-inline-size: 0; overflow: hidden; border: 1px solid var(--color-border-base); border-radius: var(--radius-2xl); background: var(--color-bg-element); }
.markdown__code-header { display: flex; align-items: center; justify-content: space-between; gap: var(--space-3); padding: var(--space-2) var(--space-4); border-bottom: 1px solid var(--color-border-base); font-family: var(--font-mono); font-size: var(--text-xs); color: var(--color-text-secondary); }
.markdown__code-header button { display: flex; align-items: center; gap: var(--space-2); border: 0; border-radius: var(--radius-sm); background: transparent; color: inherit; padding: calc(var(--space-1) * 1.5); cursor: pointer; font: inherit; }
.markdown__code-header button:hover { background: var(--color-bg-surface); color: var(--color-text-primary); }
.markdown__code-header button:focus-visible { outline: 2px solid var(--color-border-focus); outline-offset: 2px; }
.markdown__quote { margin: 0; padding-inline-start: var(--space-4); border-inline-start: 3px solid var(--color-border-strong); color: var(--color-text-secondary); overflow-wrap: anywhere; white-space: pre-line; }
.markdown__code {
  margin: 0;
  padding: var(--space-4);
  overflow: auto;
  font-family: var(--font-mono);
  font-size: var(--text-base);
  line-height: 1.6;
}

.markdown__code code {
  font-family: inherit;
  white-space: pre;
}
.code-token--keyword { color: var(--color-code-keyword); }
.code-token--string { color: var(--color-code-string); }
.code-token--number { color: var(--color-code-number); }
.code-token--comment { color: var(--color-text-tertiary); font-style: italic; }
:global([data-theme='dark'] .markdown__code-block) { background: var(--color-bg-surface); }
</style>
