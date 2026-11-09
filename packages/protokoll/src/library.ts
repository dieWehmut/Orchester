export interface LibraryStateDto {
  revision: number
  projects: { id: string; name: string; folders: string[] }[]
  sections: { id: string; name: string }[]
  sessions: {
    id: string
    title: string | null
    pinned: boolean
    unread: boolean
    archived: boolean
    project_id: string | null
    section_id: string | null
  }[]
}
