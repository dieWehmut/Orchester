<script setup lang="ts">
import { ref, watch } from 'vue'
import { AppDialog, AppInput, AppButton } from '@orchester/design'
import { Folder, FolderPlus, X } from '@lucide/vue'
import { useI18n } from '../../i18n'
import { folderName, normalizeFolder, sameFolder, type LocalProject } from './project-folders'
const props = defineProps<{ open: boolean; project: LocalProject | null; busy?: boolean; error?: string | null }>()
const emit = defineEmits<{ 'update:open': [boolean]; save: [LocalProject]; remove: [string] }>()
const { t } = useI18n()
const name = ref(''), folders = ref<string[]>([]), adding = ref(false), path = ref(''), validation = ref(''), removing = ref(false)
watch(() => [props.open, props.project] as const, () => {
  if (!props.open) return
  name.value = props.project?.name ?? ''; folders.value = [...(props.project?.folders ?? [])]
  adding.value = false; path.value = ''; validation.value = ''; removing.value = false
}, { immediate: true })
function add() {
  const value = normalizeFolder(path.value)
  if (!value) { validation.value = t('projectLibrary.absolutePathError'); return }
  if (folders.value.some(folder => sameFolder(folder, value))) { validation.value = t('projectLibrary.duplicateFolder'); return }
  folders.value.push(value); adding.value = false; path.value = ''; validation.value = ''
}
function save() {
  if (props.busy) return
  if ([...name.value.trim()].length > 120 || /[\u0000-\u001f\u007f-\u009f]/u.test(name.value)) { validation.value = t('library.nameRequired'); return }
  if (!name.value.trim() || !folders.value.length) { validation.value = t('projectLibrary.requiredError'); return }
  if (adding.value && path.value.trim()) { validation.value = t('projectLibrary.pendingFolder'); return }
  validation.value = ''
  emit('save', { id: props.project?.id ?? crypto.randomUUID(), name: name.value.trim(), folders: [...folders.value] })
}
</script>
<template>
  <AppDialog class="project-editor" :open="open" :title="project ? t('projectLibrary.edit') : t('projectLibrary.create')" :close-label="t('projectLibrary.close')" @update:open="emit('update:open', $event)">
    <form @submit.prevent="save">
      <div class="project-name"><Folder :size="22" aria-hidden="true" /><AppInput v-model="name" :aria-label="t('projectLibrary.name')" :placeholder="t('projectLibrary.name')" :disabled="busy" /></div>
      <h3>{{ t('projectLibrary.sourceFolders') }}</h3>
      <div class="folders">
        <div v-for="(folder, index) in folders" :key="folder" class="folder-row"><Folder :size="23" aria-hidden="true" /><span class="folder-details"><strong>{{ folderName(folder) }}</strong><small>{{ folder }}</small></span><button type="button" :disabled="busy" :aria-label="t('projectLibrary.removeFolder') + folder" @click="folders.splice(index, 1)"><X :size="18" /></button></div>
        <button class="add-folder" type="button" :disabled="busy" @click="adding = !adding"><FolderPlus :size="23" aria-hidden="true" />{{ t('projectLibrary.addFolder') }}</button>
        <div v-if="adding" class="folder-entry"><label for="project-folder-path">{{ t('projectLibrary.absolutePath') }}</label><AppInput id="project-folder-path" v-model="path" :disabled="busy" @keydown.enter.prevent="add" /><div class="entry-actions"><AppButton variant="ghost" :disabled="busy" @click="adding = false; path = ''">{{ t('projectLibrary.cancel') }}</AppButton><AppButton variant="secondary" :disabled="busy" @click="add">{{ t('projectLibrary.add') }}</AppButton></div></div>
      </div>
      <p v-if="validation || error" role="alert" class="error">{{ validation || error }}</p>
      <div v-if="removing" class="remove-confirm"><p>{{ t('projectLibrary.removeConfirm') }}</p><AppButton variant="danger" :busy="busy" @click="project && emit('remove', project.id)">{{ t('projectLibrary.remove') }}</AppButton><AppButton variant="ghost" :disabled="busy" @click="removing = false">{{ t('projectLibrary.keep') }}</AppButton></div>
      <footer><AppButton v-if="project" variant="danger" :disabled="busy" @click="removing = true">{{ t('projectLibrary.removeLocal') }}</AppButton><span class="spacer" /><AppButton variant="ghost" :disabled="busy" @click="emit('update:open', false)">{{ t('projectLibrary.cancel') }}</AppButton><AppButton type="submit" :busy="busy">{{ t('projectLibrary.save') }}</AppButton></footer>
    </form>
  </AppDialog>
</template>
<style scoped>
.project-editor :deep(.app-dialog) { inline-size: min(640px, 100%); border-radius: 24px; }
.project-editor :deep(.app-dialog__header) { border: 0; padding: var(--space-6) calc(var(--space-1) * 6.5) var(--space-3); }
.project-editor :deep(.app-dialog__title) { font-size: 24px; font-weight: 600; }
.project-editor :deep(.app-dialog__close) { border: 0; font-size: 0; background: transparent; }
.project-editor :deep(.app-dialog__close)::before { content: '\00d7'; font-size: 26px; line-height: 1; }
.project-editor :deep(.app-dialog__body) { padding: var(--space-4) calc(var(--space-1) * 6.5) var(--space-6); }
.project-name { display: flex; align-items: center; border: 1px solid var(--color-border-control); border-radius: 12px; overflow: hidden; background: var(--color-bg-input); }
.project-name:focus-within { border-color: var(--color-accent); }
.project-name > svg { margin: 0 var(--space-4); flex-shrink: 0; }
.project-name { background: var(--color-bg-elevated); }
.project-name :deep(input) { border: 0; border-inline-start: 1px solid var(--color-border-base); border-radius: 0; min-height: 48px; }
.project-name :deep(input) { background: transparent; }
h3 { font-size: 15px; margin: var(--space-5) 0 var(--space-3); }
.folders { border: 1px solid var(--color-border-base); border-radius: 12px; overflow: hidden; }
.folder-row, .add-folder { display: flex; align-items: center; gap: calc(var(--space-1) * 3.5); padding: calc(var(--space-1) * 3.5) var(--space-4); }
.folder-row { border-bottom: 1px solid var(--color-border-base); }
.folder-row > svg, .add-folder > svg { color: var(--color-text-tertiary); flex-shrink: 0; }
.folder-details { display: grid; min-width: 0; flex: 1; gap: calc(var(--space-1) * 0.75); }
.folder-details strong { font-weight: 400; }
.folder-details small { color: var(--color-text-tertiary); overflow-wrap: anywhere; }
button { color: inherit; font: inherit; background: transparent; border: 0; cursor: pointer; }
.folder-row button { min-height: 32px; min-width: 32px; color: var(--color-text-tertiary); }
.add-folder { width: 100%; text-align: start; }
.add-folder:hover { background: var(--color-bg-element); }
.folder-entry { padding: var(--space-4); display: grid; gap: calc(var(--space-1) * 2.5); border-top: 1px solid var(--color-border-base); }
.entry-actions, footer { display: flex; justify-content: flex-end; gap: var(--space-3); }
footer { margin-top: var(--space-6); flex-wrap: wrap; align-items: center; }
footer :deep(.app-button) { border-radius: 12px; }
footer :deep(.app-button[type='submit']) { background: var(--color-text-primary); color: var(--color-bg-base); border-color: transparent; }
footer :deep(.app-button--danger) { background: color-mix(in srgb, var(--color-status-error) 9%, transparent); color: var(--color-status-error); border-color: transparent; }
.spacer { flex: 1; }
.error { color: var(--color-status-error); font-size: 13px; }
.remove-confirm { margin-top: var(--space-4); font-size: 13px; }
</style>
