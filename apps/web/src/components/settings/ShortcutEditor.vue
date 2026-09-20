<script setup lang="ts">
/**
 * The keyboard-shortcut editor.
 *
 * It renders what the registry holds rather than a list of its own, so a
 * shortcut that appears here is one a mounted component answers, and one that
 * disappears really did stop being bound. Recording captures the next chord
 * through the same capture rule the registry uses, which is what lets it
 * refuse a bare key instead of silently binding away a letter.
 */
import { AppButton, AppInput } from '@orchester/design'
import type { Platform } from '@orchester/design'
import { computed, onUnmounted, ref } from 'vue'

import {
  captureShortcut,
  formatShortcut,
  type ShortcutEvent,
  type ShortcutRegistry,
} from '../../shortcuts/registry'

const props = defineProps<{
  registry: ShortcutRegistry
  platform: Platform
  searchLabel?: string
  searchPlaceholder?: string
  recordLabel?: string
  captureLabel?: string
  resetLabel?: string
  emptyLabel?: string
}>()

const query = ref('')
const recording = ref<string | null>(null)
const error = ref('')

/**
 * Bumped by the registry so a rebinding made anywhere 鈥?including by the reset
 * button 鈥?redraws the rows. Reading `list()` alone would render whatever was
 * bound at mount and quietly go stale.
 */
const revision = ref(0)
const unsubscribe = props.registry.subscribe(() => {
  revision.value += 1
})
onUnmounted(unsubscribe)

interface EditorRow {
  id: string
  label: string
  group: string
  keys: readonly string[]
}

const rows = computed<readonly EditorRow[]>(() => {
  void revision.value
  return props.registry.list().map((shortcut) => ({
    id: shortcut.id,
    label: shortcut.label,
    group: shortcut.group,
    keys: props.registry.effectiveKeys(shortcut.id) ?? shortcut.keys,
  }))
})

const matches = computed<readonly EditorRow[]>(() => {
  const needle = query.value.trim().toLowerCase()
  if (needle.length === 0) return rows.value
  return rows.value.filter(
    (row) =>
      row.label.toLowerCase().includes(needle) ||
      row.group.toLowerCase().includes(needle) ||
      formatShortcut(row.keys, props.platform).toLowerCase().includes(needle),
  )
})

/** Groups keep the registration order, which is the order components mounted. */
const groups = computed(() => {
  const seen: string[] = []
  for (const row of matches.value) if (!seen.includes(row.group)) seen.push(row.group)
  return seen.map((group) => ({
    name: group,
    rows: matches.value.filter((row) => row.group === group),
  }))
})

function beginRecording(id: string): void {
  error.value = ''
  recording.value = recording.value === id ? null : id
}

function onCaptureKeydown(id: string, event: KeyboardEvent): void {
  if (event.key === 'Escape') {
    recording.value = null
    return
  }

  const captured = captureShortcut(event as ShortcutEvent, props.platform)
  if (!captured) return

  try {
    props.registry.rebind(id, captured)
  } catch (thrown) {
    // A chord that is already taken keeps its owner: the reader is told which
    // one, rather than the app silently stealing a working shortcut.
    error.value = thrown instanceof Error ? thrown.message : String(thrown)
  }
  recording.value = null
}

function resetAll(): void {
  error.value = ''
  props.registry.resetAll()
}
</script>

<template>
  <section class="shortcut-editor" data-shortcut-editor>
    <header class="shortcut-editor__head">
      <AppInput
        v-model="query"
        type="search"
        class="shortcut-editor__search"
        data-shortcut-search
        :aria-label="props.searchLabel ?? 'Search shortcuts'"
        :placeholder="props.searchPlaceholder ?? 'Search shortcuts'"
      />
      <AppButton variant="secondary" size="sm" data-shortcut-reset @click="resetAll">
        {{ props.resetLabel ?? 'Reset all' }}
      </AppButton>
    </header>

    <p v-if="error" class="shortcut-editor__error" data-shortcut-error role="alert">{{ error }}</p>
    <p v-if="matches.length === 0" class="shortcut-editor__empty" data-shortcut-empty role="status">
      {{ props.emptyLabel ?? 'No shortcut matches that.' }}
    </p>

    <div v-for="group in groups" :key="group.name" class="shortcut-editor__group" :data-shortcut-group="group.name">
      <h3>{{ group.name }}</h3>
      <ul class="shortcut-editor__list">
        <li
          v-for="row in group.rows"
          :key="row.id"
          class="shortcut-editor__row"
          :data-shortcut-row="row.id"
        >
          <span class="shortcut-editor__label">{{ row.label }}</span>
          <kbd class="shortcut-editor__keys" data-shortcut-keys>
            {{ formatShortcut(row.keys, props.platform) }}
          </kbd>
          <AppButton
            v-if="recording !== row.id"
            variant="ghost"
            size="sm"
            data-shortcut-record
            :aria-label="`${props.recordLabel ?? 'Change'} ${row.label}`"
            @click="beginRecording(row.id)"
          >
            {{ props.recordLabel ?? 'Change' }}
          </AppButton>
          <input
            v-else
            class="shortcut-editor__capture"
            data-shortcut-capture
            type="text"
            readonly
            :aria-label="props.captureLabel ?? 'Press the new shortcut'"
            :placeholder="props.captureLabel ?? 'Press the new keys'"
            @keydown.prevent="onCaptureKeydown(row.id, $event)"
          />
        </li>
      </ul>
    </div>
  </section>
</template>

<style scoped>
.shortcut-editor {
  display: grid;
  gap: var(--space-3);
}

.shortcut-editor__head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.shortcut-editor__search {
  flex: 1;
}

.shortcut-editor__error {
  margin: 0;
  color: var(--color-intent-danger-text);
  font-size: var(--text-sm);
}

.shortcut-editor__empty {
  margin: 0;
  color: var(--color-text-tertiary);
  font-size: var(--text-sm);
}

.shortcut-editor__group h3 {
  margin: 0 0 var(--space-1);
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
  font-weight: var(--weight-medium);
}

.shortcut-editor__list {
  display: grid;
  gap: 2px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.shortcut-editor__row {
  display: flex;
  /* Same floor as the settings navigation row: the density row height tightens
     the visual, but the target stays at the section 7 floor even in compact. */
  min-block-size: max(var(--density-row-height), var(--hit-target-min, 32px));
  align-items: center;
  gap: var(--space-3);
}

.shortcut-editor__label {
  flex: 1;
  font-size: var(--text-sm);
}

.shortcut-editor__keys {
  padding: 2px var(--space-2);
  border: 1px solid var(--color-border-base);
  border-radius: var(--radius-xs);
  background: var(--color-bg-surface);
  color: var(--color-text-secondary);
  font-family: var(--font-mono);
  font-size: var(--text-xs);
}

.shortcut-editor__capture {
  inline-size: 10rem;
  min-block-size: var(--control-height-sm, 1.75rem);
  border: 1px dashed var(--color-border-focus);
  border-radius: var(--radius-xs);
  background: var(--color-bg-base);
  color: var(--color-text-secondary);
  font-size: var(--text-sm);
  text-align: center;
}
</style>
