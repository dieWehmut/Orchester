<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useI18n } from '../../i18n'
const { t } = useI18n()
import { AppButton, AppInput, AppSwitch } from '@orchester/design'
import { ChevronDown, ChevronRight, Trash2, Plus } from '@lucide/vue'
import type { ProviderEditorModelDto as ProviderEditorModel } from '@orchester/protokoll'
const props = defineProps<{ model: ProviderEditorModel; index: number; disabled?: boolean }>()
const emit = defineEmits<{ remove: [] }>()
const container = ref<HTMLDetailsElement | null>(null)
const expanded = ref(props.index === 0 || !props.model.model)
onMounted(() => { if (!props.model.model) container.value?.querySelector<HTMLInputElement>('input')?.focus() })
function customize(): void { props.model.reasoning_levels = ['off', 'low', 'medium', 'high', 'max'].map(level => ({ level, api_value: level === 'off' ? null : level })) }
function capacity(field: 'context_window' | 'max_output_tokens', event: Event): void {
  const input = event.target as HTMLInputElement
  const value = input.value.trim().toUpperCase()
  const match = /^(\d+(?:\.\d+)?)([KM]?)$/.exec(value)
  const number = match ? Math.round(Number(match[1]) * (match[2] === 'M' ? 1_000_000 : match[2] === 'K' ? 1_000 : 1)) : 0
  input.setCustomValidity(value && (!Number.isSafeInteger(number) || number <= 0) ? t('settings.providerEditor.invalidCapacity') : '')
  if (number > 0 && Number.isSafeInteger(number)) props.model[field] = number
  else if (!value) delete props.model[field]
}
function reasoning(enabled: boolean): void {
  props.model.reasoning_levels = enabled
    ? ['off', 'low', 'medium', 'high', 'max'].map(level => ({ level, api_value: level === 'off' ? null : level }))
    : []
}
</script>
<template>
  <details ref="container" class="model-editor" :open="expanded" @toggle="expanded = container?.open ?? false">
    <summary>
      <ChevronRight class="closed" :size="18" /><ChevronDown class="opened" :size="18" />
      <span>{{ model.display_name || model.model || t('settings.providerEditor.modelNumber', { index: String(index + 1) }) }}</span>
      <button type="button" :disabled="disabled" :aria-label="t('settings.providerEditor.removeModel', { index: String(index + 1) })" @click.prevent="emit('remove')"><Trash2 :size="18" /></button>
    </summary>
    <div class="model-editor__body">
      <div class="model-editor__pair">
        <label>{{ t('settings.providerEditor.modelId') }}<AppInput v-model="model.model" required :disabled="disabled" :placeholder="t('settings.providerEditor.modelId')" /></label>
        <label>{{ t('settings.providerEditor.displayName') }}<AppInput :model-value="model.display_name ?? ''" @update:model-value="model.display_name = $event" :disabled="disabled" :placeholder="t('settings.providerEditor.displayName')" /></label>
      </div>
      <div class="model-editor__pair">
        <label>{{ t('settings.providerEditor.contextWindow') }}<input class="model-editor__capacity" :value="model.context_window" :disabled="disabled" :placeholder="t('settings.providerEditor.exampleContext')" inputmode="decimal" @change="capacity('context_window', $event)" /></label>
        <label>{{ t('settings.providerEditor.maxOutput') }}<input class="model-editor__capacity" :value="model.max_output_tokens" :disabled="disabled" :placeholder="t('settings.providerEditor.exampleOutput')" inputmode="decimal" @change="capacity('max_output_tokens', $event)" /></label>
      </div>
      <fieldset :disabled="disabled"><legend>{{ t('settings.providerEditor.inputTypes') }}</legend><label><input v-model="model.input_types" type="checkbox" value="text" />{{ t('settings.providerEditor.text') }}</label><label><input v-model="model.input_types" type="checkbox" value="image" />{{ t('settings.providerEditor.image') }}</label></fieldset>
      <AppSwitch class="model-editor__reasoning" :label="t('settings.providerEditor.reasoning')" :model-value="model.reasoning_levels == null || model.reasoning_levels.length > 0" :disabled="disabled ?? false" @update:model-value="reasoning" />
      <div v-if="model.reasoning_levels == null" class="model-editor__toggle"><span>{{ t('settings.providerEditor.defaultLevels') }}</span><AppButton type="button" variant="ghost" size="sm" :disabled="disabled" @click="customize">{{ t('settings.providerEditor.customizeLevels') }}</AppButton></div>
      <template v-if="model.reasoning_levels?.length">
        <div class="model-editor__level-head"><span>{{ t('settings.providerEditor.level') }}</span><span>{{ t('settings.providerEditor.apiValue') }}</span></div>
        <div v-for="(level, i) in model.reasoning_levels" :key="i" class="model-editor__level">
          <AppInput v-model="level.level" required :disabled="disabled" :aria-label="t('settings.providerEditor.level')" />
          <AppInput :model-value="level.api_value ?? ''" :disabled="disabled || level.level === 'off'" :placeholder="level.level === 'off' ? t('settings.providerEditor.off') : t('settings.providerEditor.apiValue')" @update:model-value="level.api_value = $event || null" />
          <button type="button" :disabled="disabled" :aria-label="t('settings.providerEditor.removeLevel', { level: level.level })" @click="model.reasoning_levels.splice(i, 1)"><Trash2 :size="17" /></button>
        </div>
        <AppButton type="button" variant="secondary" size="sm" :disabled="disabled" @click="model.reasoning_levels.push({ level: '', api_value: '' })"><Plus :size="16" />{{ t('settings.providerEditor.addLevel') }}</AppButton>
      </template>
    </div>
  </details>
</template>
<style scoped>
.model-editor { border: 1px solid var(--color-border-base); border-radius: 20px; padding: var(--space-3); }
summary { display: flex; align-items: center; gap: var(--space-2); cursor: pointer; list-style: none; min-height: 40px; }
summary::-webkit-details-marker { display: none; } summary span { flex: 1; overflow-wrap: anywhere; }
.opened, details[open] .closed { display: none; } details[open] .opened { display: block; }
button { display: inline-flex; align-items: center; justify-content: center; border: 0; background: transparent; color: var(--color-text-secondary); min-width: 36px; min-height: 36px; cursor: pointer; }
.model-editor__body { display: grid; gap: var(--space-4); padding: var(--space-3) var(--space-1) var(--space-1); }
.model-editor__pair, .model-editor__level-head { display: grid; grid-template-columns: repeat(2,minmax(0,1fr)); gap: var(--space-3); }
.model-editor__pair label { display: grid; gap: var(--space-2); min-width: 0; }
.model-editor__level { display: grid; grid-template-columns: minmax(0,1fr) minmax(0,1fr) 36px; gap: var(--space-3); }
.model-editor__level-head { color: var(--color-text-secondary); padding-right: calc(var(--space-1) * 11.5); }
fieldset { display: flex; gap: var(--space-5); border: 0; padding: 0; margin: 0; } legend { margin-bottom: var(--space-3); color: var(--color-text-secondary); } fieldset label { display: inline-flex; align-items: center; gap: var(--space-2); }
.model-editor__toggle { display: flex; align-items: center; justify-content: space-between; }
.model-editor__reasoning { display: flex; flex-direction: row-reverse; justify-content: space-between; width: 100%; }
.model-editor__capacity { display: block; min-height: var(--control-height-md); padding: 0 var(--space-3); border: 1px solid var(--color-border-control); border-radius: var(--radius-sm); background: var(--color-bg-input); color: var(--color-text-primary); font: inherit; line-height: var(--leading-normal); }
.model-editor__capacity::placeholder { color: var(--color-text-tertiary); }
.model-editor__capacity:hover:not(:disabled) { border-color: var(--color-border-control-hover); }
.model-editor__capacity:focus-visible { border-color: var(--color-accent); }
.model-editor__capacity:disabled { cursor: not-allowed; opacity: .55; }
.model-editor__pair input { min-width: 0; width: 100%; box-sizing: border-box; }
@media(max-width: 540px) { .model-editor__pair { grid-template-columns: minmax(0,1fr); } }
</style>
