import { describe,it,expect,vi,afterEach } from 'vitest'
import { normalizeAddress, RemoteClient, newIdempotencyKey } from '../src/client'
afterEach(()=>vi.unstubAllGlobals())
describe('remote connection',()=>{
 it('normalizes LAN IP and preserves explicit secure origins',()=>{expect(normalizeAddress('192.168.1.20')).toBe('http://192.168.1.20:7337');expect(normalizeAddress('https://host.example')).toBe('https://host.example')})
 it.each(['192.168.1.20:80','localhost:80','[::1]:80'])('preserves explicitly supplied HTTP port 80 in %s',value=>expect(normalizeAddress(value)).toBe(`http://${value.slice(0,-3)}`))
 it.each(['file:///tmp','https://user:secret@host','http://host/path','http://host#token','http://host?token=x'])('rejects unsafe address %s',value=>expect(()=>normalizeAddress(value)).toThrow())
 it('pairs and keeps token in header rather than URL',async()=>{const fetcher=vi.fn().mockResolvedValueOnce({status:200,text:async()=>JSON.stringify({token:'secret'})}).mockResolvedValueOnce({status:200,text:async()=>JSON.stringify({agents:[]})});vi.stubGlobal('fetch',fetcher);const client=new RemoteClient('http://host:7337');await client.pair('code','phone');await client.request('/agents');expect(fetcher.mock.calls[1]?.[0]).toBe('http://host:7337/remote/v1/agents');expect(fetcher.mock.calls[1]?.[1].headers.Authorization).toBe('Bearer secret')})
})

it('accepts empty successful deletion responses',async()=>{vi.stubGlobal('fetch',vi.fn().mockResolvedValue(new Response(null,{status:204})));expect(await new RemoteClient('http://host').request('/runs/id','DELETE')).toBeUndefined()})
it('preserves authentication errors for non JSON bodies',async()=>{vi.stubGlobal('fetch',vi.fn().mockResolvedValue(new Response('Unauthorized',{status:401})));await expect(new RemoteClient('http://host').request('/runs')).rejects.toMatchObject({status:401})})
it('explains unexpected HTML service responses',async()=>{vi.stubGlobal('fetch',vi.fn().mockResolvedValue(new Response('<html>proxy</html>')));await expect(new RemoteClient('http://host').request('/info')).rejects.toThrow('无法识别')})

it('creates idempotency keys without secure-context randomUUID',()=>{
 const fill=vi.fn((bytes:Uint8Array)=>{bytes.fill(171);return bytes})
 vi.stubGlobal('crypto',{getRandomValues:fill})
 expect(newIdempotencyKey()).toBe('ab'.repeat(16))
 expect(fill).toHaveBeenCalledOnce()
})
