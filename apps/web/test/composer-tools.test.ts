import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, describe, expect, it, vi } from 'vitest'
import RunComposer from '../src/components/run/RunComposer.vue'
import McpDialog from '../src/components/run/McpDialog.vue'
import SketchDialog from '../src/components/run/SketchDialog.vue'
import { workspaceAttachmentContent } from '../src/components/run/workspace-attachment'
import { makeSketchAttachment, sketchSvg, type SketchStroke } from '../src/components/run/sketch'
import { createAppStores } from '../src/stores/app'
import { createHttpClient } from '../src/api/http'
import type { ComposerApi, ComposerContext } from '../src/api/composer'

const mounted: ReturnType<typeof mount>[] = []
afterEach(() => { mounted.splice(0).forEach(w => w.unmount()); document.body.innerHTML = ''; vi.restoreAllMocks() })
const response = (value: unknown) => new Response(JSON.stringify(value), { headers: { 'content-type': 'application/json' } })
async function click(selector: string): Promise<void> { (document.querySelector(selector) as HTMLButtonElement).click(); await flushPromises() }

describe('composer additions', () => {
  it('attaches bounded workspace status with whole filenames and explicit omissions', () => {
    const changes = Array.from({ length: 500 }, (_, i) => ({ path: `folder-${i}/${'long-name-'.repeat(10)}.ts`, kind: 'untracked', staged: false, unstaged: true }))
    changes.push({ path: 'important.ts', kind: 'modified', staged: true, unstaged: true })
    const content = workspaceAttachmentContent({ schema_version: 1, branch: 'main', changes, branch_changes: ['review.ts'] }, 'Project')
    expect(content.length).toBeLessThanOrEqual(16000)
    expect(content).toContain('Working changes: 501; branch changes: 1')
    expect(content).toContain('Working modified: "important.ts" [staged] [unstaged]')
    expect(content).toMatch(/Omitted \d+ additional paths/)
    expect(() => workspaceAttachmentContent({ changes: [] }, 'Project')).toThrow('Invalid workspace review')
  })
  it('opens above the whole composer, skips disabled items, and restores focus without changing the draft', async () => {
    const wrapper = mount(RunComposer, { attachTo: document.body, props: { modelValue: 'Keep my draft' } }); mounted.push(wrapper)
    const form = wrapper.get('form').element
    vi.spyOn(form, 'getBoundingClientRect').mockReturnValue({ x: 30, y: 430, top: 430, left: 30, right: 730, bottom: 620, width: 700, height: 190, toJSON: () => ({}) })
    const picker = vi.spyOn(wrapper.get('input[type="file"]').element as HTMLInputElement, 'click')
    await wrapper.get('[data-composer-attach]').trigger('click'); await flushPromises()
    const menu = document.querySelector<HTMLElement>('[data-composer-add-menu]')!
    expect(menu.style.width).toBe('700px')
    expect(menu.style.bottom).toBe(`${window.innerHeight - 430 + 8}px`)
    expect(picker).not.toHaveBeenCalled()
    expect(document.activeElement?.getAttribute('data-add-action')).toBe('files')
    menu.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }))
    expect(document.activeElement?.getAttribute('data-add-action')).toBe('draw')
    menu.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })); await flushPromises()
    expect(document.querySelector('[data-composer-add-menu]')).toBeNull()
    expect(document.activeElement).toBe(wrapper.get('[data-composer-attach]').element)
    expect(wrapper.get('textarea').element.value).toBe('Keep my draft')
    await wrapper.get('[data-composer-attach]').trigger('click'); await flushPromises(); await click('[data-add-action="files"]')
    expect(document.querySelector('[role="dialog"]')?.textContent).toContain('Choose a folder')
    const files = Array.from(document.querySelectorAll<HTMLButtonElement>('[role="dialog"] button')).find(b => b.textContent === 'Choose files')!
    files.click(); await flushPromises(); expect(picker).toHaveBeenCalledTimes(1)
  })

  it('saves, pauses and completes a goal through the API and persists plan mode', async () => {
    let state: ComposerContext = { revision: 0, goal: null, plan_mode: false }
    const updates: ComposerContext[] = []
    const http = createHttpClient({ fetch: async (url, init) => {
      if (String(url).endsWith('/composer/context')) {
        if (init?.method === 'PUT') { state = { ...JSON.parse(String(init.body)), revision: state.revision + 1 }; updates.push(state) }
        return response(state)
      }
      return response(String(url).endsWith('/plugins') ? { schema_version: 1, plugins: [] } : [])
    } })
    const stores = createAppStores({ http, agentStatusStreamFactory: null }); stores.bootstrap.status.value = 'ready'
    const wrapper = mount(RunComposer, { attachTo: document.body, props: { modelValue: 'Keep draft' }, global: { plugins: [stores] } }); mounted.push(wrapper)
    await flushPromises(); await wrapper.get('[data-composer-attach]').trigger('click'); await flushPromises(); await click('[data-add-action="goal"]')
    const input = document.querySelector<HTMLTextAreaElement>('[role="dialog"] textarea')!
    input.value = 'Finish the report'; input.dispatchEvent(new Event('input', { bubbles: true }))
    document.querySelector('[role="dialog"] form')!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })); await flushPromises()
    expect(updates[0]?.goal).toEqual({ text: 'Finish the report', status: 'active' })
    await click('[aria-label="Pause goal"]'); expect(state.goal?.status).toBe('paused')
    await click('[aria-label="Mark goal complete"]'); expect(state.goal?.status).toBe('completed')
    await wrapper.get('[data-composer-attach]').trigger('click'); await flushPromises(); await click('[data-add-action="plan"]')
    expect(state.plan_mode).toBe(true)
    expect(wrapper.get('textarea').element.value).toBe('Keep draft')
    expect(wrapper.emitted('submit')).toBeUndefined()
  })

  it('keeps attachments after failed submission and clears them after acceptance', async () => {
    const wrapper = mount(RunComposer, { props: { modelValue: 'Explain', lifecycle: 'idle' } }); mounted.push(wrapper)
    const input = wrapper.get('input[type="file"]')
    const file = new File(['Hello'], 'folder/note.txt'); Object.defineProperty(file, 'text', { value: async () => 'Hello' })
    Object.defineProperty(input.element, 'files', { value: [file] })
    await input.trigger('change'); await flushPromises()
    await wrapper.setProps({ lifecycle: 'submitting' }); await wrapper.setProps({ lifecycle: 'failed' })
    expect(wrapper.find('[data-composer-attachments]').exists()).toBe(true)
    await wrapper.setProps({ lifecycle: 'submitting' }); await wrapper.setProps({ lifecycle: 'running' })
    expect(wrapper.find('[data-composer-attachments]').exists()).toBe(false)
  })

  it('keeps the drawing editor open when the combined attachment limit is exceeded', async () => {
    const wrapper = mount(RunComposer, { attachTo: document.body, props: { modelValue: 'Keep my draft' } }); mounted.push(wrapper)
    await wrapper.get('[data-composer-attach]').trigger('click'); await flushPromises(); await click('[data-add-action="draw"]')
    wrapper.getComponent(SketchDialog).vm.$emit('attach', { id: 'large', name: 'drawing.svg', content: 'x'.repeat(28000) })
    await flushPromises()
    expect(wrapper.findComponent(SketchDialog).exists()).toBe(true)
    expect(document.querySelector('[role="dialog"] [role="alert"]')).not.toBeNull()
    expect(wrapper.get('textarea').element.value).toBe('Keep my draft')
    expect(wrapper.find('[data-composer-attachments]').exists()).toBe(false)
    wrapper.getComponent(SketchDialog).vm.$emit('attach', { id: 'small', name: 'drawing.svg', content: '<svg />' })
    await flushPromises()
    expect(wrapper.findComponent(SketchDialog).exists()).toBe(false)
    expect(wrapper.get('[data-composer-attachments]').text()).toContain('drawing.svg')
  })

  it('serializes editable drawing shapes and escaped labels into the actual sent attachment', () => {
    const strokes: SketchStroke[] = [
      { kind: 'arrow', color: '#202020', points: [{ x: 10, y: 10 }, { x: 50, y: 10 }] },
      { kind: 'text', color: '#aabbcc', points: [{ x: 50, y: 50 }], text: '<script>& "label"' },
    ]
    const svg = sketchSvg(strokes)
    expect(svg).toContain('M10,10 L50,10 M36,2 L50,10 L36,18')
    expect(svg).toContain('&lt;script&gt;&amp; &quot;label&quot;')
    expect(svg).not.toContain('<script>')
    const file = makeSketchAttachment(strokes, 'drawing.svg')
    expect(file.content).toContain(svg)
    expect(decodeURIComponent(file.preview!.slice('data:image/svg+xml,'.length))).toBe(svg)
  })

  it('invokes an MCP tool only on explicit submit and attaches its visible result', async () => {
    const invoke = vi.fn(async () => ({ content: [{ type: 'text', text: 'Actual server result' }] }))
    const api = { servers: async () => [{ id: 'a', name: 'Example', transport: 'http', url: 'http://localhost/mcp' }],
      catalog: async () => ({ serverInfo: { name: 'Example', version: '1' }, tools: [{ name: 'echo', description: 'Echo text', inputSchema: { type: 'object', properties: { text: { type: 'string' } }, required: ['text'] } }], resources: [], prompts: [] }), invoke } as unknown as ComposerApi
    const wrapper = mount(McpDialog, { attachTo: document.body, props: { api, initialId: 'a' } }); mounted.push(wrapper); await flushPromises()
    await wrapper.findAll('button').find(b => b.text() === 'echo')!.trigger('click')
    expect(invoke).not.toHaveBeenCalled()
    await wrapper.get('textarea').setValue('{"text":"Test"}')
    await wrapper.get('form').trigger('submit'); await flushPromises()
    expect(invoke).toHaveBeenCalledTimes(1)
    expect(invoke.mock.calls[0]).toEqual(['a', 'tools/call', { name: 'echo', arguments: { text: 'Test' } }, expect.any(AbortSignal)])
    expect(wrapper.get('.mcp-result').text()).toContain('Actual server result')
    await wrapper.findAll('button').find(b => b.text() === 'Attach result')!.trigger('click')
    expect(wrapper.emitted('attach')?.[0]?.[0]).toMatchObject({ name: 'Example · echo', content: expect.stringContaining('Actual server result') })
  })
})
