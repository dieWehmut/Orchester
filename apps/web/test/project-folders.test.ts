import { describe, expect, it } from 'vitest'
import { folderName, normalizeFolder, sameFolder } from '../src/components/projects/project-folders'
describe('source folder paths', () => {
  it('retains absolute paths and rejects names and control characters', () => {
    expect(normalizeFolder(' /home/han/project/ ')).toBe('/home/han/project')
    expect(normalizeFolder('project')).toBe('')
    expect(normalizeFolder('/foo\nbar')).toBe('')
    expect(normalizeFolder('/')).toBe('/')
  })
  it('shows basename while preserving platform-aware duplicate checks', () => {
    expect(folderName('C:\\Work\\Project')).toBe('Project')
    expect(sameFolder('C:\\Work\\Project', 'c:/work/project')).toBe(true)
    expect(sameFolder('/Work/Project', '/work/project')).toBe(false)
  })
})
