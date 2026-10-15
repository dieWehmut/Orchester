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
import { Check, Hand, ShieldCheck, ShieldAlert } from '@lucide/vue'
import { computed, ref } from 'vue'

import { useI18n } from '../../i18n'

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
  },
)

const { t } = useI18n()

const emit = defineEmits<{
  'update:modelValue': [value: ApprovalPreset]
}>()

const PRESETS: readonly { id: ApprovalPreset; label: () => string }[] = [
  { id: 'ask', label: () => props.label ?? t('run.ask') },
  { id: 'governed', label: () => props.governLabel ?? t('run.governed') },
  { id: 'full-access', label: () => props.fullAccessLabel ?? t('run.fullAccess') },
]

const menuItems = computed<AppMenuItem[]>(() =>
  PRESETS.map((preset) => ({
    id: preset.id,
    label: preset.label(),
    disabled: preset.id === props.modelValue,
    checked: preset.id === props.modelValue,
  })),
)
const currentLabel = computed(
  () => PRESETS.find((preset) => preset.id === props.modelValue)?.label() ?? props.label,
)
const confirming = ref(false)
const isDanger = computed(() => props.modelValue === 'full-access')
const descriptions = computed<Record<string, string>>(() => ({ ask: t('add.askHelp'), governed: t('add.governedHelp'), 'full-access': t('add.fullHelp') }))

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
    <AppMenu :label="`${props.menuLabel ?? t('run.approvalPreset')}: ${currentLabel}`" :items="menuItems" align="start" placement="top" @select="choose">
      <template #header>{{ t('add.approvalHeading') }}</template>
      <template #item="{ item }">
        <span class="approval-preset__option" :data-preset="item.id">
          <Hand v-if="item.id === 'ask'" :size="20" /><ShieldCheck v-else-if="item.id === 'governed'" :size="20" /><ShieldAlert v-else :size="20" />
          <span class="approval-preset__option-copy"><strong>{{ item.label }}</strong><small>{{ descriptions[item.id] }}</small></span>
          <Check v-if="item.checked" :size="17" aria-hidden="true" />
        </span>
      </template>
      <template #trigger>
        <span class="approval-preset__trigger" data-approval-preset-trigger>
          <ShieldAlert v-if="isDanger" :size="19" aria-hidden="true" /><ShieldCheck v-else :size="19" aria-hidden="true" />
          <span data-approval-preset-label>{{ currentLabel }}</span>
        </span>
      </template>
    </AppMenu>

    <div
      v-if="confirming"
      class="approval-preset__confirm"
      data-approval-preset-confirm
      role="alertdialog"
      :aria-label="props.confirmTitle ?? t('run.fullAccessTitle')"
    >
      <p class="approval-preset__confirm-title">{{ props.confirmTitle ?? t('run.fullAccessTitle') }}</p>
      <p class="approval-preset__confirm-copy">{{ props.confirmDescription ?? t('run.fullAccessDescription') }}</p>
      <div class="approval-preset__confirm-actions">
        <AppButton
          variant="ghost"
          size="sm"
          data-approval-preset-confirm="cancel"
          @click="cancel"
        >
          {{ props.confirmCancelLabel ?? t('run.fullAccessCancel') }}
        </AppButton>
        <AppButton
          variant="danger"
          size="sm"
          data-approval-preset-confirm="accept"
          @click="accept"
        >
          {{ props.confirmAcceptLabel ?? t('run.fullAccessAccept') }}
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
.approval-preset__option { display: flex; width: 100%; gap: 10px; align-items: center; }
.approval-preset__option > svg { flex-shrink: 0; }
.approval-preset__option[data-preset='full-access'] { color: var(--composer-access, var(--color-intent-warning-text)); }
.approval-preset__option-copy { display: grid; flex: 1; gap: 2px; white-space: normal; }
.approval-preset__option-copy strong { font-size: 13px; font-weight: 500; }
.approval-preset__option-copy small { font-size: 12px; opacity: .7; }
.approval-preset :deep(.app-menu__surface) { inline-size: min(370px, calc(100vw - 40px)); border-radius: 16px; padding: 6px; }
.approval-preset :deep(.app-menu__item) { padding: 10px; border-radius: 12px; }
.approval-preset :deep(.app-menu__item:disabled) { opacity: 1; }

.approval-preset :deep(.app-menu__trigger) {
  padding-inline: var(--space-2);
  border: 0;
  border-radius: 10px;
  background: transparent;
  font-size: var(--text-sm);
}

.approval-preset :deep(.app-menu__trigger:hover) {
  background: var(--workspace-hover, var(--color-bg-element));
}

.approval-preset[data-approval-preset-danger='true'] .approval-preset__trigger {
  color: var(--composer-access, var(--color-intent-warning-text));
}

.approval-preset__confirm {
  position: absolute;
  z-index: var(--z-popover, 30);
  inset-block-end: calc(100% + var(--space-2));
  inset-inline-start: 0;
  display: grid;
  inline-size: min(22rem, calc(100vw - 48px));
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

