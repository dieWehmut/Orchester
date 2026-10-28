<script setup lang="ts">
import { Bell, ChevronDown, ChevronRight, Clock3, Moon, PawPrint, Puzzle, Search, Settings2, SquarePen, Sun } from '@lucide/vue'
import { nextTick, ref } from 'vue'

withDefaults(defineProps<{
  productName: string
  workspaceName?: string | null
  newSessionLabel: string
  projectsLabel: string
  sessionsLabel: string
  fleetLabel: string
  accountName?: string | null
  accountHint?: string | null
  settingsLabel?: string
  companionLabel?: string
  searchLabel?: string
  attentionLabel?: string
  attentionCount?: number
  /** The other destinations. The reference keeps them in this column as rows,
   *  beside "New thread", rather than in a strip of their own. */
  schedulesLabel?: string
  pluginsLabel?: string
  themeLabel?: string
  dark?: boolean
  companionVisible?: boolean
}>(), {
  searchLabel: 'Search',
  attentionLabel: '',
  attentionCount: 0,
  schedulesLabel: '',
  pluginsLabel: '',
  themeLabel: '',
  dark: true,
  companionVisible: false,
})
const emit = defineEmits<{
  newSession: []
  openSettings: []
  toggleCompanion: []
  search: [query: string]
  openAttention: []
  openSchedules: []
  openPlugins: []
  toggleTheme: []
}>()
const collapsed = ref<Record<string, boolean>>({ pinned: false, projects: false, agents: false })
const searching = ref(false)
const query = ref('')
const searchField = ref<HTMLInputElement | null>(null)
const searchButton = ref<HTMLButtonElement | null>(null)
function openSearch(): void {
  searching.value = true
  void nextTick(() => searchField.value?.focus())
}
function closeSearch(): void {
  searching.value = false
  query.value = ''
  emit('search', '')
  void nextTick(() => searchButton.value?.focus())
}
function handleQuery(value: string): void { query.value = value; emit('search', value) }
</script>

<template>
  <div class="app-rail" data-app-rail>
    <div class="app-rail__primary" data-rail-section="primary">
      <button class="app-rail__row" type="button" data-rail-row="new-session" data-rail-action="new-session" @click="emit('newSession')">
        <SquarePen :size="17" aria-hidden="true" /><span>{{ newSessionLabel }}</span>
      </button>
      <button ref="searchButton" class="app-rail__row" type="button" :aria-label="searchLabel" :aria-expanded="searching" data-rail-action="search" @click="openSearch">
        <Search :size="17" aria-hidden="true" /><span>{{ searchLabel }}</span>
      </button>
      <button v-if="attentionCount > 0" class="app-rail__row" type="button" :aria-label="attentionLabel + ' (' + attentionCount + ')'" data-rail-action="attention" @click="emit('openAttention')">
        <Bell :size="17" aria-hidden="true" /><span>{{ attentionLabel }}</span>
        <span class="app-rail__attention-count" aria-hidden="true">{{ attentionCount }}</span>
      </button>
      <button v-if="schedulesLabel" class="app-rail__row" type="button" data-rail-action="schedules" @click="emit('openSchedules')">
        <Clock3 :size="17" aria-hidden="true" /><span>{{ schedulesLabel }}</span>
      </button>
      <button v-if="pluginsLabel" class="app-rail__row" type="button" data-rail-action="plugins" @click="emit('openPlugins')">
        <Puzzle :size="17" aria-hidden="true" /><span>{{ pluginsLabel }}</span>
      </button>
    </div>
    <div v-if="searching" class="app-rail__search" data-rail-search>
      <Search :size="15" aria-hidden="true" />
      <input ref="searchField" :value="query" type="search" :placeholder="searchLabel" :aria-label="searchLabel" @input="handleQuery(($event.target as HTMLInputElement).value)" @keydown.escape="closeSearch" />
    </div>
    <div class="app-rail__scroll" data-rail-scroll>
      <div class="app-rail__section" data-rail-section="projects" data-rail-heading="pinned">
        <button class="app-rail__heading" type="button" data-rail-disclosure="pinned" :aria-expanded="!collapsed.pinned" @click="collapsed.pinned = !collapsed.pinned">
          <ChevronRight v-if="collapsed.pinned" :size="13" aria-hidden="true" /><ChevronDown v-else :size="13" aria-hidden="true" />{{ projectsLabel }}
        </button>
        <div v-if="!collapsed.pinned" class="app-rail__entries"><slot name="projects" /></div>
      </div>
      <div class="app-rail__section" data-rail-section="fleet" data-rail-heading="agents">
        <button class="app-rail__heading" type="button" data-rail-disclosure="agents" :aria-expanded="!collapsed.agents" @click="collapsed.agents = !collapsed.agents">
          <ChevronRight v-if="collapsed.agents" :size="13" aria-hidden="true" /><ChevronDown v-else :size="13" aria-hidden="true" />{{ fleetLabel }}
        </button>
        <div v-if="!collapsed.agents" class="app-rail__entries"><slot name="fleet" /></div>
      </div>
      <div class="app-rail__section" data-rail-section="sessions" data-rail-heading="projects">
        <button class="app-rail__heading" type="button" data-rail-disclosure="projects" :aria-expanded="!collapsed.projects" @click="collapsed.projects = !collapsed.projects">
          <ChevronRight v-if="collapsed.projects" :size="13" aria-hidden="true" /><ChevronDown v-else :size="13" aria-hidden="true" />{{ sessionsLabel }}
        </button>
        <div v-if="!collapsed.projects" class="app-rail__entries"><slot name="sessions" /></div>
      </div>
    </div>
    <div class="app-rail__footer" data-rail-section="footer">
      <button class="app-rail__row" type="button" :aria-pressed="companionVisible" data-rail-action="companion" @click="emit('toggleCompanion')">
        <PawPrint :size="17" aria-hidden="true" /><span>{{ companionLabel }}</span>
      </button>
      <button class="app-rail__row" type="button" :aria-pressed="dark" data-rail-action="theme" @click="emit('toggleTheme')">
        <Moon v-if="dark" :size="17" aria-hidden="true" /><Sun v-else :size="17" aria-hidden="true" /><span>{{ themeLabel }}</span>
      </button>
      <button class="app-rail__row" type="button" data-rail-action="settings" @click="emit('openSettings')">
        <Settings2 :size="17" aria-hidden="true" /><span>{{ settingsLabel }}</span>
      </button>
    </div>
  </div>
</template>

<style scoped>
/* The reference's rail is a stack of plain rows: an icon, a label, and no
   filled box around any of them. The rows carry the whole interaction - the
   hover and the selected state - so the column reads as a list of things to do
   rather than as a toolbar with one prominent button. */
.app-rail { display: flex; flex-direction: column; block-size: 100%; min-block-size: 0; background: var(--rail-surface); backdrop-filter: var(--rail-blur); }
.app-rail__primary { display: flex; flex-direction: column; gap: 1px; padding: var(--space-2) var(--space-2) var(--space-3); }
.app-rail__row { display: flex; align-items: center; gap: var(--space-3); min-inline-size: 0; min-block-size: 34px; padding: var(--space-1) var(--space-2); border: 0; border-radius: 8px; background: transparent; color: var(--color-text-primary); font: inherit; font-size: var(--text-sm); cursor: pointer; text-align: start; }
.app-rail__row > svg { flex: 0 0 auto; color: var(--color-text-secondary); }
.app-rail__row > span:not(.app-rail__attention-count) { min-inline-size: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.app-rail__scroll { flex: 1; min-block-size: 0; overflow-y: auto; overflow-x: hidden; scrollbar-width: thin; scrollbar-color: var(--workspace-seam, var(--color-border-subtle)) transparent; }
.app-rail__section { min-inline-size: 0; padding: 0 var(--space-2) var(--space-3); }
/* A group heading is a label, not a control: the reference states "Threads"
   in the same small grey type whether or not it can be folded, so the chevron
   is drawn only on hover and the text never moves. */
.app-rail__heading { display: flex; align-items: center; gap: var(--space-1); inline-size: 100%; min-block-size: 28px; padding: var(--space-1) var(--space-2); border: 0; border-radius: 6px; background: transparent; color: var(--color-text-tertiary); font: inherit; font-size: var(--text-xs); text-align: start; cursor: pointer; }
.app-rail__heading > svg { opacity: 0; transition: opacity var(--transition-fast) var(--ease-out); }
.app-rail__entries { min-inline-size: 0; }
.app-rail__heading:hover { color: var(--color-text-secondary); }
.app-rail__heading:hover > svg { opacity: 1; }
.app-rail__row:hover, .app-rail__heading:hover { background: var(--workspace-hover, var(--color-bg-element)); }
.app-rail__row:hover { color: var(--color-text-primary); }
.app-rail__heading:focus-visible, .app-rail__row:focus-visible { outline: 2px solid var(--color-border-focus); outline-offset: -2px; }
.app-rail__search { display: flex; align-items: center; gap: var(--space-2); margin: 0 var(--space-2) var(--space-2); padding: var(--space-2); border: 1px solid var(--color-border-base); border-radius: 8px; color: var(--color-text-tertiary); }
.app-rail__search:focus-within { border-color: var(--color-border-focus); }
.app-rail__search input { inline-size: 100%; min-inline-size: 0; border: 0; outline: none; background: transparent; color: var(--color-text-primary); font: inherit; font-size: var(--text-sm); }
.app-rail__attention-count { flex: 0 0 auto; margin-inline-start: auto; min-inline-size: 16px; border-radius: 8px; background: var(--color-accent); color: var(--color-bg-app); font-size: 10px; line-height: 16px; text-align: center; }
/* The destinations the reference keeps at the foot of the column: settings and
   the two appearance switches. They are rows like every other one, so the
   column never grows a second vocabulary for "a place you can go". */
.app-rail__footer { display: flex; flex-direction: column; gap: 1px; padding: var(--space-2); }
.app-rail__footer .app-rail__row > span { color: var(--color-text-secondary); }
.app-rail__footer .app-rail__row:hover > span { color: var(--color-text-primary); }
</style>
