<script setup lang="ts">
/**
 * The code preview on the appearance screen.
 *
 * Two columns of the same snippet, one before and one after, so a theme change
 * can be judged on the thing this product actually renders: monospaced text on
 * a surface, with added and removed lines marked the way a diff marks them.
 *
 * The diff is computed rather than passed in, because the caller's job is the
 * theme, not the diffing.
 */
import { computed } from 'vue'

const props = withDefaults(
  defineProps<{
    /** The snippet as it was, one line per entry. */
    before: readonly string[]
    /** The snippet as it is now. */
    after: readonly string[]
    title?: string
    hint?: string
  }>(),
  { title: '', hint: '' },
)

/** Longest common prefix, so unchanged head lines stay unmarked. */
function commonPrefixLength(a: readonly string[], b: readonly string[]): number {
  const limit = Math.min(a.length, b.length)
  let index = 0
  while (index < limit && a[index] === b[index]) index += 1
  return index
}

function commonSuffixLength(a: readonly string[], b: readonly string[], prefix: number): number {
  const limit = Math.min(a.length, b.length) - prefix
  let index = 0
  while (index < limit && a[a.length - 1 - index] === b[b.length - 1 - index]) index += 1
  return index
}

const beforeLines = computed(() => {
  const prefix = commonPrefixLength(props.before, props.after)
  const suffix = commonSuffixLength(props.before, props.after, prefix)
  return props.before.map((text, index) => ({
    number: index + 1,
    text,
    state: index < prefix || index >= props.before.length - suffix ? 'unchanged' : 'removed',
  }))
})

const afterLines = computed(() => {
  const prefix = commonPrefixLength(props.before, props.after)
  const suffix = commonSuffixLength(props.before, props.after, prefix)
  return props.after.map((text, index) => ({
    number: index + 1,
    text,
    state: index < prefix || index >= props.after.length - suffix ? 'unchanged' : 'added',
  }))
})
</script>

<template>
  <section class="code-preview" data-code-preview :aria-label="title || 'Code preview'">
    <header v-if="title || hint" class="code-preview__head" data-code-head>
      <span class="code-preview__title">{{ title }}</span>
      <span v-if="hint" class="code-preview__hint">{{ hint }}</span>
    </header>
    <div class="code-preview__columns">
      <ol
        class="code-preview__column"
        data-code-column="before"
        data-code-gutter="removed"
      >
        <li
          v-for="line in beforeLines"
          :key="line.number"
          class="code-preview__line"
          :data-code-line="line.state"
          :data-code-number="line.number"
        >
          <span class="code-preview__number" aria-hidden="true">{{ line.number }}</span>
          <span class="code-preview__marker" aria-hidden="true">{{
            line.state === 'removed' ? '-' : ''
          }}</span>
          <code>{{ line.text }}</code>
        </li>
      </ol>
      <ol
        class="code-preview__column"
        data-code-column="after"
        data-code-gutter="added"
      >
        <li
          v-for="line in afterLines"
          :key="line.number"
          class="code-preview__line"
          :data-code-line="line.state"
          :data-code-number="line.number"
        >
          <span class="code-preview__number" aria-hidden="true">{{ line.number }}</span>
          <span class="code-preview__marker" aria-hidden="true">{{
            line.state === 'added' ? '+' : ''
          }}</span>
          <code>{{ line.text }}</code>
        </li>
      </ol>
    </div>
  </section>
</template>

<style scoped>
.code-preview {
  overflow: hidden;
  border: 1px solid var(--color-border-default);
  border-radius: var(--radius-md);
  background: var(--color-surface-base);
}

.code-preview__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  padding: var(--space-3) var(--space-4);
  border-block-end: 1px solid var(--color-border-default);
}

.code-preview__title {
  color: var(--color-text-primary);
  font-size: var(--text-sm);
  font-weight: var(--weight-medium);
}

.code-preview__hint {
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
}

.code-preview__columns {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.code-preview__column {
  margin: 0;
  padding: var(--space-2) 0;
  list-style: none;
  position: relative;
}

/* The full-height rail down the edge of each pane, in the colour of the change
   that pane carries: the reference draws one, so a reader knows which side they
   are looking at before they read a line number. */
.code-preview__column[data-code-gutter='removed']::before,
.code-preview__column[data-code-gutter='added']::before {
  content: '';
  position: absolute;
  inset-block: 0;
  inset-inline-start: 0;
  inline-size: 3px;
}

.code-preview__column[data-code-gutter='removed']::before {
  background: var(--color-intent-danger-border);
}

.code-preview__column[data-code-gutter='added']::before {
  background: var(--color-intent-success-border);
}

.code-preview__column + .code-preview__column {
  border-inline-start: 1px solid var(--color-border-default);
}

.code-preview__line {
  display: grid;
  grid-template-columns: 2rem 1rem minmax(0, 1fr);
  align-items: center;
  gap: var(--space-2);
  padding-inline: var(--space-2) var(--space-4);
  font-family: var(--font-mono);
  font-size: var(--text-xs);
  line-height: 1.6;
}

.code-preview__line code {
  min-inline-size: 0;
  overflow: hidden;
  color: var(--color-text-secondary);
  text-overflow: ellipsis;
  white-space: pre;
}

.code-preview__number {
  color: var(--color-text-tertiary);
  font-family: var(--font-mono);
  text-align: end;
}

.code-preview__marker {
  font-family: var(--font-mono);
  text-align: center;
}

.code-preview__line[data-code-line='removed'] {
  background: var(--color-intent-danger-surface);
}

.code-preview__line[data-code-line='removed'] .code-preview__marker,
.code-preview__line[data-code-line='removed'] code {
  color: var(--color-intent-danger-text);
}

.code-preview__line[data-code-line='added'] {
  background: var(--color-intent-success-surface);
}

.code-preview__line[data-code-line='added'] .code-preview__marker,
.code-preview__line[data-code-line='added'] code {
  color: var(--color-intent-success-text);
}

@media (max-width: 900px) {
  .code-preview__columns {
    grid-template-columns: minmax(0, 1fr);
  }

  .code-preview__column + .code-preview__column {
    border-block-start: 1px solid var(--color-border-default);
    border-inline-start: 0;
  }
}
</style>
