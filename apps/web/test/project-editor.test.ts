import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import ProjectEditor from '../src/components/projects/ProjectEditor.vue'
const project = { id: 'p1', name: 'Example', folders: ['/home/han/example'] }
describe('project editor', () => {
  it('saves a copy of project source folders', async () => {
    const wrapper = mount(ProjectEditor, { props: { open: true, project } })
    await wrapper.get('form').trigger('submit')
    expect(wrapper.emitted('save')).toEqual([[project]])
    expect((wrapper.emitted('save')![0]![0] as typeof project).folders).not.toBe(project.folders)
    wrapper.unmount()
  })
  it('confirms grouping removal before emitting', async () => {
    const wrapper = mount(ProjectEditor, { props: { open: true, project } })
    await wrapper.get('footer .app-button--danger').trigger('click')
    expect(wrapper.emitted('remove')).toBeUndefined()
    expect(wrapper.get('.remove-confirm').text()).toContain('remain on disk')
    await wrapper.get('.remove-confirm .app-button--danger').trigger('click')
    expect(wrapper.emitted('remove')).toEqual([['p1']])
    wrapper.unmount()
  })
  it('rejects duplicate source folders without changing saved folders', async () => {
    const wrapper = mount(ProjectEditor, { props: { open: true, project } })
    await wrapper.get('.add-folder').trigger('click')
    await wrapper.get('#project-folder-path').setValue('/home/han/example/')
    await wrapper.get('.entry-actions .app-button--secondary').trigger('click')
    expect(wrapper.get('[role="alert"]').text()).toContain('already included')
    expect(wrapper.findAll('.folder-row')).toHaveLength(1)
    wrapper.unmount()
  })
  it('explains an invalid project name before attempting a save', async () => {
    const wrapper = mount(ProjectEditor, { props: { open: true, project } })
    await wrapper.get('.project-name input').setValue('x'.repeat(121))
    await wrapper.get('form').trigger('submit')
    expect(wrapper.emitted('save')).toBeUndefined()
    expect(wrapper.get('[role="alert"]').text()).toContain('120 characters')
    wrapper.unmount()
  })
})
