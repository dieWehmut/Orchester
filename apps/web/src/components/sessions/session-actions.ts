export interface SessionDestination { id: string; name: string }
export interface SessionMetadata { pinned?: boolean; unread: boolean; archived: boolean; project_id: string | null; section_id: string | null }
export interface SessionAction {
  id: string
  action: 'rename' | 'pin' | 'unread' | 'project' | 'section' | 'share' | 'copy-link' | 'copy-markdown' | 'copy-directory' | 'open-window' | 'archive'
  value?: string | null
}
