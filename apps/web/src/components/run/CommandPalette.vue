<script setup lang="ts">
/**
 * The `/` command palette.
 *
 * Opened and driven by keystrokes, so it cannot rely on a pointer to be
 * complete: every entry says what it does, the highlight moves with the arrow
 * keys, Enter opens it and Escape closes it. When there is nothing to show it
 * says which nothing it is — still loading, or nothing matched.
 */
import { computed, ref, watch } from 'vue'

export interface CommandEntry {
  id: string
  name: string
  description: string
}

const props = withDefaults(
  defineProps<{
    open?: boolean
    commands?: readonly CommandEntry[]
    query?: string
    loading?: boolean
    emptyLabel?: string
    loadingLabel?: string
  }>(),
  {
    open: false,
    commands: () => [],
    query: '',
    loading: false,
    emptyLabel: 'No command matches that.',
    loadingLabel: 'Loading commands…',
  },
)

const emit = defineEmits<{
  select: [id: string]
  close: []
}>()

const normalizedQuery = computed(() => props.query.trim().replace(/^\//, '').toLowerCase())
const matches = computed(() =>
  props.commands.filter((command) =>
    normalizedQuery.value.length === 0
      ? true
      : command.name.toLowerCase().includes(normalizedQuery.value) ||
        command.description.toLowerCase().includes(normalizedQuery.value),
  ),
)

const activeIndex = ref(0)
watch(matches, () => {
  activeIndex.value = 0
})

function move(step: number): void {
  const count = matches.value.length
  if (count === 0) return
  activeIndex.value = (activeIndex.value + step + count) % count
}

function onKeydown(event: KeyboardEvent): void {
  if (event.key === 'ArrowDown') {
    event.preventDefault()
    move(1)
  } else if (event.key === 'ArrowUp') {
    event.preventDefault()
    move(-1)
  } else if (event.key === 'Enter') {
    event.preventDefault()
    const active = matches.value[activeIndex.value]
    if (active) emit('select', active.id)
  } else if (event.key === 'Escape') {
    event.preventDefault()
    emit('close')
  }
}
</script>

<template>
  <div
    v-if="props.open"
    class="command-palette"
    data-command-palette
    role="listbox"
    tabindex="-1"
    @keydown="onKeydown"
  >
    <p v-if="props.loading" class="command-palette__note" data-command-loading>
      {{ loadingLabel }}
    </p>
    <p
      v-else-if="matches.length === 0"
      class="command-palette__note"
      data-command-empty
    >
      {{ emptyLabel }}
    </p>
    <ul v-else class="command-palette__list">
      <li
        v-for="(command, index) in matches"
        :key="command.id"
        class="command-palette__item"
        data-command-item
        role="option"
        :data-command-active="index === activeIndex"
        :aria-selected="index === activeIndex"
        @click="emit('select', command.id)"
      >
        <span class="command-palette__name">{{ command.name }}</span>
        <span class="command-palette__description">{{ command.description }}</span>
      </li>
    </ul>
  </div>
</template>

<style scoped>
.command-palette {
  display: grid;
  gap: var(--space-1);
  max-block-size: 18rem;
  overflow: auto;
  margin-inline: auto;
  inline-size: min(100%, var(--composer-max-width));
  padding: var(--space-2);
  border: 1px solid var(--color-border-base);
  border-radius: var(--radius-md);
  background: var(--color-bg-surface);
  box-shadow: var(--shadow-300);
}

.command-palette__list {
  display: grid;
  gap: 2px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.command-palette__item {
  display: grid;
  gap: var(--space-1);
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius-xs);
  cursor: pointer;
}

.command-palette__item[data-command-active='true'] {
  background: var(--color-bg-element);
}

.command-palette__name {
  font-family: var(--font-mono);
  font-size: var(--text-sm);
}

.command-palette__description {
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
}

.command-palette__note {
  margin: 0;
  padding: var(--space-3);
  color: var(--color-text-tertiary);
  font-size: var(--text-sm);
}
</style>

