import { invoke, isTauri } from '@tauri-apps/api/core'

export async function openConversationWindow(id: string, url: string): Promise<void> {
  if (!/^s-[a-fA-F0-9]{32}$/.test(id)) throw new Error('Invalid conversation identifier')
  if (isTauri()) {
    await invoke('open_conversation_window', { id })
    return
  }
  const target = new URL(url, window.location.href)
  if (target.origin !== window.location.origin || target.pathname !== '/workspace'
    || target.searchParams.get('session') !== id || target.hash) {
    throw new Error('Invalid conversation URL')
  }
  const opened = window.open('about:blank', '_blank')
  if (!opened) throw new Error('Conversation window could not be opened')
  try {
    opened.opener = null
    const policy = opened.document.createElement('meta')
    policy.name = 'referrer'
    policy.content = 'no-referrer'
    opened.document.head.appendChild(policy)
    opened.location.replace(target.href)
  } catch (error) {
    opened.close()
    throw error
  }
}
