import { Capacitor, CapacitorHttp } from '@capacitor/core'
export type Project = { id: string; name: string; path: string }
export type FolderEntry = { name: string; path: string; type: 'directory' | 'file' }
export type FolderPage = { path: string; parent_path: string | null; roots: string[]; entries: FolderEntry[]; next_offset: number | null }
export type Run = { project_id?: string; workspace?: string; created_at?: number; id: string; agent: string; prompt: string; state: string; session_id?: string | null; error_code?: string | null }
export type Events = { events: { seq: number; event: unknown }[]; next_cursor: number; state: string }
export function normalizeAddress(input: string): string {
  const value = input.trim()
  const url = new URL(value.includes('://') ? value : `http://${value}`)
  if (!['http:', 'https:'].includes(url.protocol) || url.username || url.password || url.search || url.hash || url.pathname !== '/') throw new Error('请输入电脑 IP、主机名或完整 HTTP(S) 地址，不要包含路径或凭据')
  // URL removes the default :80 port. Preserve that explicit choice rather
  // than silently connecting to Orchester's default on a different port.
  const explicitPort = /^(?:\[[^\]]+\]|[^:/?#]+):\d+\/?$/.test(value)
  if (!url.port && !value.includes('://') && !explicitPort) url.port = '7337'
  return url.origin
}
export class RemoteError extends Error { constructor(readonly status: number, message: string) { super(message) } }
export class RemoteClient {
  constructor(readonly address: string, private token = '') {}
  async request<T>(path: string, method = 'GET', data?: unknown): Promise<T> {
    const headers: Record<string,string> = { 'Content-Type': 'application/json' }
    if (this.token) headers.Authorization = `Bearer ${this.token}`
    const url = `${this.address}/remote/v1${path}`
    let status: number, body: unknown
    if (Capacitor.isNativePlatform()) {
      const response = await CapacitorHttp.request({ url, method, headers, data, responseType:'json', connectTimeout:10000, readTimeout:20000, disableRedirects:true })
      status = response.status; body = response.data
    } else {
      const response = await fetch(url, { method, headers, ...(data === undefined ? {} : { body: JSON.stringify(data) }), signal: AbortSignal.timeout(20000), redirect:'error', credentials:'omit' })
      status = response.status; const raw = await response.text(); try { body = raw ? JSON.parse(raw) : undefined } catch { body = raw }
    }
    if (status < 200 || status >= 300) {
      let message = `请求失败 (${status})，请检查电脑服务和配对码`
      if (status === 401) message = path === '/pair' ? '配对码不正确或已失效，请在电脑生成新配对码' : '配对已过期，请输入新配对码重新配对'
      if (status === 409) message = '项目目录或任务状态已改变，请刷新后重试'
      if (status === 403 && path.startsWith('/folders')) message = '此目录不在电脑允许浏览的范围内，或没有访问权限'
      if (status === 413 && path.startsWith('/folders')) message = '此目录项过多，请输入更具体的子目录路径'
      if (status === 404 && (path.startsWith('/folders') || path.startsWith('/projects'))) message = '目录或项目不存在；请检查路径和电脑版服务版本'
      if (status === 429) message = path === '/pair'
        ? '暂时无法配对，请在电脑生成新配对码；仍失败时检查已配对设备数量'
        : '任务数量达到上限，请停止运行中的任务或删除已结束的记录后重试'
      if (status === 404 && path.startsWith('/runs/')) message = '任务不存在或已被其他设备删除，请刷新任务记录'
      throw new RemoteError(status, message)
    }
    if (status !== 204 && typeof body === 'string') throw new Error('电脑返回了无法识别的响应，请检查地址和服务端口')
    return body as T
  }
  async pair(code: string, deviceName: string) {
    const result = await this.request<{token:string; device_id:string; expires_at:number}>('/pair','POST',{code,device_name:deviceName})
    this.token = result.token
    return result
  }
}

export function newIdempotencyKey(): string {
  return Array.from(crypto.getRandomValues(new Uint8Array(16)), byte => byte.toString(16).padStart(2, '0')).join('')
}
