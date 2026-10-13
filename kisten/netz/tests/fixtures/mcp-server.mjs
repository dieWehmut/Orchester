import readline from 'node:readline'
let initialized = false
const lines = readline.createInterface({ input: process.stdin })
lines.on('line', line => {
  const request = JSON.parse(line)
  if (request.method === 'notifications/initialized') { initialized = true; return }
  let result
  if (request.method === 'initialize') result = { protocolVersion: '2025-11-25', capabilities: { tools: {} }, serverInfo: { name: 'stdio-fixture', version: '1' } }
  else {
    if (!initialized) throw new Error('initialized notification must precede calls')
    if (request.method === 'tools/list') result = { tools: [{ name: 'echo', description: 'Echo a provided string', inputSchema: { type: 'object', properties: { text: { type: 'string' } }, required: ['text'] } }] }
    else if (request.method === 'tools/call') result = { content: [{ type: 'text', text: request.params.arguments.text }] }
    else throw new Error('unsupported method')
  }
  process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id: request.id, result }) + '\n')
})
