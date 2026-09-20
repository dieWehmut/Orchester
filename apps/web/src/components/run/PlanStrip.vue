<script setup lang="ts">
/**
 * The plan strip: the agent's plan as progress, not as a list.
 *
 * A blocked plan is the one state that asks the user for something, so it has
 * its own treatment and its own flag rather than being folded into "active".
 * The details are collapsed by default; the strip's job is to say where the run
 * is, and the list is there for the reader who wants it.
 */
import type { UiValidation } from '@orchester/protokoll'
import { ListChecks, TriangleAlert } from '@lucide/vue'
import { computed, ref } from 'vue'

import { useI18n } from '../../i18n'

export interface PlanTodo {
  text: string
  completed: boolean
}

const props = withDefaults(
  defineProps<{
    todos?: readonly PlanTodo[]
    validation?: UiValidation | null
    /** Set while the plan cannot move without the user. */
    blocked?: boolean
    expandLabel?: string
    collapseLabel?: string
  }>(),
  {
    todos: () => [],
    validation: null,
    blocked: false,
  },
)

const { t } = useI18n()

type PlanState = 'idle' | 'active' | 'blocked' | 'done'
type SegmentState = 'done' | 'current' | 'pending'

const completedCount = computed(() => props.todos.filter((todo) => todo.completed).length)
const expanded = ref(false)

const planState = computed<PlanState>(() => {
  if (props.todos.length === 0) return 'idle'
  if (props.blocked) return 'blocked'
  if (completedCount.value === props.todos.length) return 'done'
  return 'active'
})

/** The step in hand: the first unfinished one, or nothing once the plan is done. */
const currentTodo = computed(() =>
  planState.value === 'done' ? null : props.todos.find((todo) => !todo.completed) ?? null,
)

function segmentState(index: number): SegmentState {
  if (props.todos[index]?.completed) return 'done'
  // Only the first unfinished step is current; later ones are still waiting.
  if (index === completedCount.value) return 'current'
  return 'pending'
}
</script>

<template>
  <div
    v-if="planState !== 'idle'"
    class="plan-strip"
    data-plan-strip
    :data-plan-state="planState"
    :data-plan-needs-input="planState === 'blocked'"
  >
    <div class="plan-strip__row">
      <span class="plan-strip__icon" aria-hidden="true">
        <TriangleAlert v-if="planState === 'blocked'" :size="14" />
        <ListChecks v-else :size="14" />
      </span>

      <span
        v-if="currentTodo"
        class="plan-strip__current"
        data-plan-current
      >
        {{ currentTodo.text }}
      </span>
      <span v-else class="plan-strip__current" data-plan-current-complete>
        {{ t('run.planComplete') }}
      </span>

      <span
        class="plan-strip__progress"
        data-plan-progress
        role="progressbar"
        :aria-valuenow="completedCount"
        :aria-valuemax="props.todos.length"
      >
        <span
          v-for="(todo, index) in props.todos"
          :key="todo.text + index"
          class="plan-strip__segment"
          data-plan-segment
          :data-plan-segment-state="segmentState(index)"
        />
      </span>

      <button
        class="plan-strip__toggle"
        type="button"
        data-plan-toggle
        :aria-expanded="expanded"
        @click="expanded = !expanded"
      >
        {{ expanded ? props.collapseLabel ?? t('run.hidePlan') : props.expandLabel ?? t('run.showPlan') }}
      </button>
    </div>

    <div v-if="expanded" class="plan-strip__details" data-plan-details>
      <ul class="plan-strip__list">
        <li
          v-for="(todo, index) in props.todos"
          :key="todo.text + index"
          class="plan-strip__item"
          data-plan-item
          :data-plan-item-state="todo.completed ? 'done' : 'pending'"
        >
          {{ todo.text }}
        </li>
      </ul>
      <p v-if="props.validation" class="plan-strip__validation" data-plan-validation>
        {{ props.validation.summary }}
      </p>
    </div>
  </div>
</template>

<style scoped>
.plan-strip {
  display: grid;
  gap: var(--space-2);
  margin-inline: auto;
  max-inline-size: var(--composer-max-width);
  padding: var(--space-2) var(--space-4);
  color: var(--color-text-secondary);
  font-size: var(--text-xs);
}

.plan-strip[data-plan-state='blocked'] {
  border-inline-start: 2px solid var(--color-intent-warning-border);
  color: var(--color-intent-warning-text);
}

.plan-strip__row {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.plan-strip__icon {
  display: inline-flex;
  flex: none;
}

.plan-strip__current {
  overflow: hidden;
  min-inline-size: 0;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.plan-strip__progress {
  display: flex;
  flex: 1;
  gap: 2px;
  min-inline-size: 3rem;
}

.plan-strip__segment {
  flex: 1;
  block-size: 3px;
  border-radius: 2px;
  background: var(--color-border-base);
}

.plan-strip__segment[data-plan-segment-state='done'] {
  background: var(--color-intent-success-solid);
}

.plan-strip__segment[data-plan-segment-state='current'] {
  background: var(--color-accent);
}

.plan-strip__toggle {
  flex: none;
  min-block-size: var(--hit-target-min, 32px);
  border: 0;
  background: none;
  color: inherit;
  cursor: pointer;
  font: inherit;
  text-decoration: underline;
}

.plan-strip__list {
  display: grid;
  gap: var(--space-1);
  margin: 0;
  padding: 0;
  list-style: none;
}

.plan-strip__item[data-plan-item-state='done'] {
  color: var(--color-text-tertiary);
  text-decoration: line-through;
}

.plan-strip__validation {
  margin: 0;
  color: var(--color-text-tertiary);
}
</style>

