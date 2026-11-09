import type { LibraryStateDto } from '@orchester/protokoll'
import type { HttpClient } from './http'

export type LibrarySession = LibraryStateDto['sessions'][number]
export type LibraryProject = LibraryStateDto['projects'][number]
export interface LibraryApi {
  load(): Promise<LibraryStateDto>
  save(state: LibraryStateDto): Promise<LibraryStateDto>
}
export function createLibraryApi(http: HttpClient): LibraryApi {
  return {
    load: () => http.get('/library'),
    save: state => http.put('/library', state),
  }
}
