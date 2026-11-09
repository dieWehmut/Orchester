import { afterEach, describe, expect, it, vi } from 'vitest'
import { openConversationWindow } from '../src/platform/conversation-window'

vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => false, invoke: vi.fn() }))
const id = 's-0123456789abcdef0123456789abcdef'
afterEach(() => vi.unstubAllGlobals())

describe('conversation browser windows', () => {
  function setup(blocked = false) {
    const policy = { name: '', content: '' }
    const opened = {
      opener: {} as unknown,
      document: { createElement: vi.fn(() => policy), head: { appendChild: vi.fn() } },
      location: { replace: vi.fn() }, close: vi.fn(),
    }
    const open = vi.fn(() => blocked ? null : opened)
    vi.stubGlobal('window', { location: { href: 'http://localhost/workspace', origin: 'http://localhost' }, open })
    return { open, opened, policy }
  }
  it('opens under user gesture, removes opener and suppresses referrer before navigation', async () => {
    const { open, opened, policy } = setup()
    await openConversationWindow(id, `/workspace?session=${id}`)
    expect(open).toHaveBeenCalledWith('about:blank', '_blank')
    expect(opened.opener).toBeNull()
    expect(policy).toEqual({ name: 'referrer', content: 'no-referrer' })
    expect(opened.location.replace).toHaveBeenCalledWith(`http://localhost/workspace?session=${id}`)
  })
  it('rejects blocked popups', async () => {
    setup(true)
    await expect(openConversationWindow(id, `/workspace?session=${id}`)).rejects.toThrow('could not be opened')
  })
  it('rejects malicious IDs and external URLs before opening', async () => {
    const { open } = setup()
    await expect(openConversationWindow('../settings', '/workspace')).rejects.toThrow()
    await expect(openConversationWindow(id, `https://evil.example/workspace?session=${id}`)).rejects.toThrow()
    expect(open).not.toHaveBeenCalled()
  })
  it('closes the blank window if navigation fails', async () => {
    const { opened } = setup()
    opened.location.replace.mockImplementation(() => { throw new Error('navigation failed') })
    await expect(openConversationWindow(id, `/workspace?session=${id}`)).rejects.toThrow('navigation failed')
    expect(opened.close).toHaveBeenCalledOnce()
  })
})
