import { computed, ref } from 'vue'
import { useAppStores } from '../stores/app'
import { sessionPreference } from '../stores/library'
import { createSessionsApi } from '../api/sessions'
import type { LibraryProject } from '../api/library'
import type { SessionAction } from '../components/sessions/session-actions'
import { conversationMarkdown, conversationUrl } from '../components/sessions/session-export'
import { openConversationWindow } from '../platform/conversation-window'
import { useI18n } from '../i18n'

export function useLibraryActions() {
  const { library, http, sessions } = useAppStores()
  const { t } = useI18n()
  const nameDialog = ref<{ id: string; kind: 'rename' | 'section' } | null>(null)
  const name = ref(''), validation = ref(''), notice = ref('')
  const projectOpen = ref(false), project = ref<LibraryProject | null>(null), assignProjectTo = ref<string | null>(null)
  const share = ref<{ id: string; title: string; markdown: string } | null>(null)
  const metadata = library.metadata
  const namedItems = computed(() => sessions.items.value.map(row => ({ ...row, title: metadata.value[row.id]?.title ?? row.title })))
  const titleFor = (id: string) => metadata.value[id]?.title ?? sessions.items.value.find(item => item.id === id)?.title ?? ''
  async function copy(text: string): Promise<void> {
    try {
      if (!navigator.clipboard) throw new Error('Clipboard unavailable')
      await navigator.clipboard.writeText(text)
      notice.value = t('library.copied')
    } catch { notice.value = t('library.copyFailed') }
  }
  function editProject(id: string | null, assignTo: string | null = null): void {
    project.value = library.state.value.projects.find(row => row.id === id) ?? null
    assignProjectTo.value = assignTo
    library.error.value = null
    projectOpen.value = true
  }
  async function saveProject(value: LibraryProject): Promise<void> {
    const target = assignProjectTo.value
    if (await library.mutate(state => {
      const index = state.projects.findIndex(item => item.id === value.id)
      if (index >= 0) state.projects[index] = value
      else state.projects.push(value)
      if (target) { const row = sessionPreference(state, target); row.project_id = value.id; row.section_id = null }
    })) { projectOpen.value = false; notice.value = t('library.saved') }
  }
  async function removeProject(id: string): Promise<void> {
    if (await library.mutate(state => {
      state.projects = state.projects.filter(row => row.id !== id)
      state.sessions.forEach(row => { if (row.project_id === id) row.project_id = null })
    })) projectOpen.value = false
  }
  async function saveName(): Promise<void> {
    if (library.busy.value) return
    const dialog = nameDialog.value
    const value = name.value.trim()
    if (!dialog || !value || [...value].length > 120 || /[\u0000-\u001f\u007f-\u009f]/u.test(value)) { validation.value = t('library.nameRequired'); return }
    const sectionId = crypto.randomUUID()
    const ok = await library.mutate(state => {
      const row = sessionPreference(state, dialog.id)
      if (dialog.kind === 'rename') row.title = value
      else { state.sections.push({ id: sectionId, name: value }); row.section_id = sectionId; row.project_id = null }
    })
    if (ok) { nameDialog.value = null; notice.value = t('library.saved') }
  }
  function localLink(id: string): string { return conversationUrl(id, window.location.href) }
  async function action(command: SessionAction): Promise<void> {
    const { id, value } = command
    notice.value = ''; validation.value = ''; library.error.value = null
    if (command.action === 'rename' || (command.action === 'section' && value === '__new__')) {
      nameDialog.value = { id, kind: command.action === 'rename' ? 'rename' : 'section' }
      name.value = command.action === 'rename' ? titleFor(id) : ''
      return
    }
    if (command.action === 'project' && value === '__new__') { editProject(null, id); return }
    const field = command.action === 'pin' ? 'pinned' : command.action === 'archive' ? 'archived' : 'unread'
    const nextState = !metadata.value[id]?.[field]
    try {
      switch (command.action) {
        case 'pin': await library.updateSession(id, row => { row.pinned = nextState }); break
        case 'unread': await library.updateSession(id, row => { row.unread = nextState }); break
        case 'archive': await library.updateSession(id, row => { row.archived = nextState }); break
        case 'project': await library.updateSession(id, row => { row.project_id = value ?? null; if (value) row.section_id = null }); break
        case 'section': await library.updateSession(id, row => { row.section_id = value ?? null; if (value) row.project_id = null }); break
        case 'copy-link': await copy(localLink(id)); break
        case 'copy-directory': {
          const workspace = await http.get<{ path: string }>('/library/workspace')
          await copy(workspace.path)
          break
        }
        case 'open-window': await openConversationWindow(id, localLink(id)); break
        case 'copy-markdown':
        case 'share': {
          const detail = await createSessionsApi(http).detail(id)
          const title = titleFor(id) || detail.title
          const markdown = conversationMarkdown(detail, title)
          if (command.action === 'copy-markdown') await copy(markdown)
          else share.value = { id, title, markdown }
          break
        }
      }
    } catch { notice.value = t('library.failed') }
  }
  function download(): void {
    if (!share.value) return
    const url = URL.createObjectURL(new Blob([share.value.markdown], { type: 'text/markdown;charset=utf-8' }))
    const anchor = document.createElement('a')
    anchor.href = url
    anchor.download = `${share.value.title.replace(/[<>:"/\\|?*\x00-\x1f]/g, '_').slice(0, 80) || 'conversation'}.md`
    anchor.click()
    setTimeout(() => URL.revokeObjectURL(url), 1000)
  }
  return { library, namedItems, nameDialog, name, validation, notice, projectOpen, project, share, action, editProject, saveProject, removeProject, saveName, copy, download, localLink }
}
