<script setup lang="ts">
/**
 * The approval preset, named rather than described.
 *
 * 'Full access' is the one choice that can hand an agent the machine without
 * stopping to ask, so selecting it opens a confirmation instead of applying it.
 * The control reports the live scope in the DOM so the composer can paint the
 * danger intent without every caller re-deriving which preset is risky.
 */
import { AppButton, AppMenu, type AppMenuItem } from '@orchester/design'
import { ShieldCheck } from '@lucide/vue'
import { computed, ref } from 'vue'

export type ApprovalPreset = 'ask' | 'governed' | 'full-access'

const props = withDefaults(
  defineProps<{
    modelValue?: ApprovalPreset
    label?: string
    governLabel?: string
    fullAccessLabel?: string
    confirmTitle?: string
    confirmDescription?: string
    confirmAcceptLabel?: string
    confirmCancelLabel?: string
    /** The name of the control that opens the preset menu. */
    menuLabel?: string
  }>(),
  {
    modelValue: 'ask',
    label: 'Ask',
    governLabel: 'Governed',
    fullAccessLabel: 'Full access',
    confirmTitle: 'Give the run full access?',
    confirmDescription:
      'Full access lets the agent change files and reach the network without stopping to ask. It applies to the next run.',
    confirmAcceptLabel: 'Use full access',
    confirmCancelLabel: 'Keep the current preset',
    menuLabel: 'Approval preset',
  },
)

const emit = defineEmits<{
  'update:modelValue': [value: ApprovalPreset]
}>()

const PRESETS: readonly { id: ApprovalPreset; label: () => string }[] = [
  { id: 'ask', label: () => props.label },
  { id: 'governed', label: () => props.governLabel },
  { id: 'full-access', label: () => props.fullAccessLabel },
]

const menuItems = computed<AppMenuItem[]>(() =>
  PRESETS.map((preset) => ({
    id: preset.id,
    label: preset.label(),
    disabled: preset.id === props.modelValue,
  })),
)
const currentLabel = computed(
  () => PRESETS.find((preset) => preset.id === props.modelValue)?.label() ?? props.label,
)
const confirming = ref(false)
const isDanger = computed(() => props.modelValue === 'full-access')

function choose(id: string): void {
  const preset = id as ApprovalPreset
  if (preset === 'full-access') {
    confirming.value = true
    return
  }
  emit('update:modelValue', preset)
}

function accept(): void {
  confirming.value = false
  emit('update:modelValue', 'full-access')
}

function cancel(): void {
  confirming.value = false
}
</script>

<template>
  <div
    class="approval-preset"
    data-approval-preset
    :data-approval-preset-state="props.modelValue"
    :data-approval-preset-danger="isDanger"
  >
    <AppMenu :label="`${menuLabel}: ${currentLabel}`" :items="menuItems" align="end" @select="choose">
      <template #trigger>
        <span class="approval-preset__trigger" data-approval-preset-trigger>
          <ShieldCheck :size="15" aria-hidden="true" />
          <span data-approval-preset-label>{{ currentLabel }}</span>
        </span>
      </template>
    </AppMenu>

    <div
      v-if="confirming"
      class="approval-preset__confirm"
      data-approval-preset-confirm
      role="alertdialog"
      :aria-label="confirmTitle"
    >
      <p class="approval-preset__confirm-title">{{ confirmTitle }}</p>
      <p class="approval-preset__confirm-copy">{{ confirmDescription }}</p>
      <div class="approval-preset__confirm-actions">
        <AppButton
          variant="ghost"
          size="sm"
          data-approval-preset-confirm="cancel"
          @click="cancel"
        >
          {{ confirmCancelLabel }}
        </AppButton>
        <AppButton
          variant="danger"
          size="sm"
          data-approval-preset-confirm="accept"
          @click="accept"
        >
          {{ confirmAcceptLabel }}
        </AppButton>
      </div>
    </div>
  </div>
</template>

<style scoped>
.approval-preset {
  position: relative;
  display: inline-flex;
  align-items: center;
}

.approval-preset__trigger {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
}

.approval-preset[data-approval-preset-danger='true'] .approval-preset__trigger {
  color: var(--color-intent-danger-text);
}

.approval-preset__confirm {
  position: absolute;
  z-index: var(--z-popover, 30);
  inset-block-end: calc(100% + var(--space-2));
  inset-inline-end: 0;
  display: grid;
  inline-size: 22rem;
  gap: var(--space-2);
  padding: var(--space-3);
  border: 1px solid var(--color-intent-danger-border);
  border-radius: var(--radius-md);
  background: var(--color-intent-danger-surface);
  box-shadow: var(--shadow-300);
}

.approval-preset__confirm-title {
  margin: 0;
  color: var(--color-intent-danger-text);
  font-size: var(--text-sm);
  font-weight: var(--weight-semibold);
}

.approval-preset__confirm-copy {
  margin: 0;
  color: var(--color-text-secondary);
  font-size: var(--text-xs);
  line-height: var(--leading-normal);
}

.approval-preset__confirm-actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-2);
}
</style>

