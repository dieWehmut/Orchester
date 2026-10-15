import type { HttpClient } from './http'

export interface ComposerGoal { text: string; status: 'active' | 'paused' | 'completed' }
export interface ComposerContext { revision: number; goal: ComposerGoal | null; plan_mode: boolean }
export interface McpServerInput {
  name: string; transport: 'stdio' | 'http'; url: string; token_env: string
  command: string; args: string[]; env: Record<string, string>
}
export interface McpServer extends McpServerInput { id: string }
export interface McpTool { name: string; title?: string; description?: string; inputSchema: Record<string, unknown> }
export interface McpResource { name: string; title?: string; description?: string; uri: string }
export interface McpPrompt { name: string; title?: string; description?: string; arguments?: { name: string; description?: string; required?: boolean }[] }
export interface McpCatalog { serverInfo: { name: string; version: string }; tools: McpTool[]; resources: McpResource[]; prompts: McpPrompt[] }
const record = (v: unknown): v is Record<string, unknown> => !!v && typeof v === 'object' && !Array.isArray(v)
function context(value: unknown): ComposerContext {
  if (!record(value) || !Number.isSafeInteger(value.revision) || Number(value.revision) < 0 || typeof value.plan_mode !== 'boolean'
    || (value.goal !== null && (!record(value.goal) || typeof value.goal.text !== 'string' || !['active', 'paused', 'completed'].includes(String(value.goal.status))))) throw new Error('Invalid composer context')
  return value as unknown as ComposerContext
}
function server(value: unknown): McpServer {
  if (!record(value) || !['id','name','url','command','token_env'].every(k => typeof value[k] === 'string') || !['http','stdio'].includes(String(value.transport))
    || !Array.isArray(value.args) || !value.args.every(a => typeof a === 'string') || !record(value.env) || !Object.values(value.env).every(v => typeof v === 'string')) throw new Error('Invalid MCP server')
  return value as unknown as McpServer
}
function catalog(value: unknown): McpCatalog {
  if (!record(value) || !record(value.serverInfo) || typeof value.serverInfo.name !== 'string' || typeof value.serverInfo.version !== 'string'
    || !Array.isArray(value.tools) || !value.tools.every(v => record(v) && typeof v.name === 'string' && record(v.inputSchema))
    || !Array.isArray(value.resources) || !value.resources.every(v => record(v) && typeof v.name === 'string' && typeof v.uri === 'string')
    || !Array.isArray(value.prompts) || !value.prompts.every(v => record(v) && typeof v.name === 'string' && (v.arguments === undefined || (Array.isArray(v.arguments) && v.arguments.every(a => record(a) && typeof a.name === 'string'))))) throw new Error('Invalid MCP catalog')
  return value as unknown as McpCatalog
}
export function createComposerApi(http: HttpClient) {
  return {
    context: async (signal?: AbortSignal) => context(await http.get<unknown>('/composer/context', signal ? { signal } : {})),
    saveContext: async (input: ComposerContext) => context(await http.put<unknown>('/composer/context', input)),
    servers: async (signal?: AbortSignal) => { const value = await http.get<unknown>('/mcp/servers', signal ? { signal } : {}); if (!Array.isArray(value)) throw new Error('Invalid MCP servers'); return value.map(server) },
    saveServer: (input: McpServerInput, id?: string) => id
      ? http.put<McpServer>(`/mcp/servers/${encodeURIComponent(id)}`, input)
      : http.post<McpServer>('/mcp/servers', input),
    removeServer: (id: string) => http.delete(`/mcp/servers/${encodeURIComponent(id)}`),
    catalog: async (id: string, signal?: AbortSignal) => catalog(await http.post<unknown>(`/mcp/servers/${encodeURIComponent(id)}/catalog`, undefined, signal ? { signal } : {})),
    invoke: (id: string, method: 'tools/call' | 'resources/read' | 'prompts/get', params: Record<string, unknown>, signal?: AbortSignal) =>
      http.post<Record<string, unknown>>(`/mcp/servers/${encodeURIComponent(id)}/invoke`, { method, params }, signal ? { signal } : {}),
  }
}
export type ComposerApi = ReturnType<typeof createComposerApi>
