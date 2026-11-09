<script setup lang="ts">
import { computed } from 'vue'
import { DropdownMenuRoot, DropdownMenuTrigger, DropdownMenuPortal, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator, DropdownMenuSub, DropdownMenuSubTrigger, DropdownMenuSubContent } from 'reka-ui'
import { Ellipsis, Pencil, Pin, Eye, FolderInput, List, Share, Copy, PanelsTopLeft, Archive, ChevronRight, Check } from '@lucide/vue'
import { useI18n } from '../../i18n'
import type { SessionAction, SessionDestination } from './session-actions'
const props = defineProps<{ id: string; pinned?: boolean | undefined; unread?: boolean | undefined; archived?: boolean | undefined; projectId?: string | null | undefined; sectionId?: string | null | undefined; projects?: readonly SessionDestination[] | undefined; sections?: readonly SessionDestination[] | undefined }>()
const emit = defineEmits<{ action: [action: SessionAction]; 'update:open': [open: boolean] }>()
const { t } = useI18n()
const groups = computed(() => [
  [{ action: 'rename', text: t('sessionMenu.rename'), icon: Pencil, shortcut: 'Ctrl+Alt+R' },
   { action: 'pin', text: props.pinned ? t('sessionMenu.unpin') : t('sessionMenu.pin'), icon: Pin, shortcut: 'Ctrl+Alt+P' },
   { action: 'unread', text: props.unread ? t('sessionMenu.markRead') : t('sessionMenu.markUnread'), icon: Eye, shortcut: 'Ctrl+Shift+U' },
   { action: 'project', text: t('sessionMenu.project'), icon: FolderInput, children: destinations(props.projects, props.projectId) },
   { action: 'section', text: t('sessionMenu.section'), icon: List, children: destinations(props.sections, props.sectionId) }],
  [{ action: 'share', text: t('sessionMenu.share'), icon: Share },
   { action: 'copy', text: t('sessionMenu.copy'), icon: Copy, children: [
     { action: 'copy-link', text: t('sessionMenu.copyLink'), shortcut: 'Ctrl+Alt+L' },
     { action: 'copy-markdown', text: t('sessionMenu.copyMarkdown') },
     { action: 'copy-directory', text: t('sessionMenu.copyDirectory'), shortcut: 'Ctrl+Shift+C' }] }],
  [{ action: 'open-window', text: t('sessionMenu.openWindow'), icon: PanelsTopLeft }],
  [{ action: 'archive', text: props.archived ? t('sessionMenu.restore') : t('sessionMenu.archive'), icon: Archive, shortcut: 'Ctrl+Shift+A' }],
])
interface Child { text: string; value?: string | null | undefined; action?: string; checked?: boolean | undefined; shortcut?: string }
function destinations(items: readonly SessionDestination[] = [], selected?: string | null | undefined): Child[] {
  return [{ text: t('sessionMenu.none'), value: null, checked: !selected }, ...items.map(item => ({ text: item.name, value: item.id, checked: item.id === selected })), { text: t('sessionMenu.create'), value: '__new__' }]
}
function run(action: string, value?: string | null | undefined) { emit('action', { id: props.id, action: action as SessionAction['action'], ...(value !== undefined ? { value } : {}) }) }
</script>
<template>
  <DropdownMenuRoot @update:open="emit('update:open', $event)">
    <DropdownMenuTrigger class="session-menu-trigger" :aria-label="t('sessionMenu.actions')" :data-session-menu="id"><Ellipsis :size="18" /></DropdownMenuTrigger>
    <DropdownMenuPortal>
      <DropdownMenuContent class="session-menu" align="start" :side-offset="4" :collision-padding="8">
        <template v-for="(group, index) in groups" :key="index">
          <DropdownMenuSeparator v-if="index" class="session-menu-separator" />
          <template v-for="item in group" :key="item.action">
            <DropdownMenuSub v-if="'children' in item && item.children">
              <DropdownMenuSubTrigger class="session-menu-item"><component :is="item.icon" :size="19" /><span>{{ item.text }}</span><ChevronRight class="session-menu-chevron" :size="16" /></DropdownMenuSubTrigger>
              <DropdownMenuPortal><DropdownMenuSubContent class="session-menu" :side-offset="6" :collision-padding="8">
                <DropdownMenuItem v-for="child in item.children" :key="('action' in child ? child.action : 'value' in child ? child.value : child.text) ?? '__none__'" class="session-menu-item" @select="run(child.action ?? item.action, 'value' in child ? child.value : undefined)">
                  <Copy v-if="item.action === 'copy'" :size="19" /><Check v-else :size="16" :style="{ opacity: 'checked' in child && child.checked ? 1 : 0 }" /><span>{{ child.text }}</span><span v-if="'shortcut' in child" class="session-menu-shortcut">{{ child.shortcut }}</span>
                </DropdownMenuItem>
              </DropdownMenuSubContent></DropdownMenuPortal>
            </DropdownMenuSub>
            <DropdownMenuItem v-else class="session-menu-item" @select="run(item.action)"><component :is="item.icon" :size="19" /><span>{{ item.text }}</span><span v-if="'shortcut' in item" class="session-menu-shortcut">{{ item.shortcut }}</span></DropdownMenuItem>
          </template>
        </template>
      </DropdownMenuContent>
    </DropdownMenuPortal>
  </DropdownMenuRoot>
</template>
<style>
.session-menu-trigger { display: grid; place-items: center; width: 28px; height: 30px; padding: 0; border: 0; border-radius: 6px; background: transparent; color: var(--color-text-secondary); cursor: pointer; }
.session-menu-trigger:focus-visible { outline: 2px solid var(--color-accent); }
.session-menu { z-index: 100; min-width: 260px; max-width: calc(100vw - var(--space-4)); padding: calc(var(--space-1) * 1.75); border: 1px solid var(--color-border-base); border-radius: 20px; background: var(--color-bg-elevated); color: var(--color-text-primary); box-shadow: 0 12px 30px #00000016; }
.session-menu-item { display: flex; align-items: center; gap: calc(var(--space-1) * 2.25); padding: calc(var(--space-1) * 2.25) calc(var(--space-1) * 2.5); border-radius: 12px; font-size: 14px; line-height: 20px; cursor: default; outline: none; user-select: none; }
.session-menu-item[data-highlighted], .session-menu-item[data-state="open"] { background: var(--workspace-hover, var(--color-bg-element)); }
.session-menu-item > svg { flex-shrink: 0; }
.session-menu-shortcut { margin-inline-start: auto; padding-inline-start: var(--space-2); color: var(--color-text-tertiary); font-size: 12px; white-space: nowrap; }
.session-menu-chevron { margin-inline-start: auto; color: var(--color-text-tertiary); }
.session-menu-separator { height: 1px; margin: calc(var(--space-1) * 1.25) calc(var(--space-1) * 2.5); background: var(--color-border-base); }
</style>
