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
import { computed } from 'vue'

import { parseMarkdown } from '../markdown'
import MarkdownSpans from './MarkdownSpans.vue'

const props = withDefaults(
  defineProps<{
    text: string
    /** Announced inside every link; the surface owns the words. */
    externalLabel?: string
  }>(),
  { externalLabel: '' },
)

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
        <MarkdownSpans :spans="block.spans" :external-label="externalLabel" />
      </h3>
      <pre
        v-else-if="block.kind === 'code'"
        class="markdown__code"
        data-markdown-code
        :data-code-language="block.language ?? ''"
      ><code>{{ block.text }}</code></pre>
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
                <MarkdownSpans :spans="cell" :external-label="externalLabel" />
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
                <MarkdownSpans :spans="cell" :external-label="externalLabel" />
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
          <MarkdownSpans :spans="item" :external-label="externalLabel" />
        </li>
      </component>
      <p v-else class="markdown__paragraph" data-markdown-paragraph>
        <MarkdownSpans :spans="block.spans" :external-label="externalLabel" />
      </p>
    </template>
  </div>
</template>

<style scoped>
.markdown {
  display: grid;
  gap: var(--space-3);
  min-inline-size: 0;
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
  font-size: var(--text-lg);
}

.markdown__heading[data-heading-level='2'] {
  font-size: var(--text-base);
}

.markdown__heading[data-heading-level='3'],
.markdown__heading[data-heading-level='4'],
.markdown__heading[data-heading-level='5'],
.markdown__heading[data-heading-level='6'] {
  font-size: var(--text-sm);
  color: var(--color-text-secondary);
}

.markdown__list {
  display: grid;
  gap: var(--space-1);
  padding-inline-start: var(--space-5);
}

.markdown__code {
  margin: 0;
  padding: var(--space-3);
  overflow: auto;
  border: 1px solid var(--color-border-base);
  border-radius: var(--radius-sm);
  background: var(--color-bg-element);
  font-family: var(--font-mono);
  font-size: var(--text-xs);
  line-height: 1.6;
}

.markdown__code code {
  font-family: inherit;
  white-space: pre;
}
</style>