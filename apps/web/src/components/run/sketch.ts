export interface SketchPoint { x: number; y: number }
export interface SketchStroke { kind: 'pen' | 'line' | 'rect' | 'arrow' | 'text'; color: string; points: SketchPoint[]; text?: string }
export interface ComposerAttachment { id: string; name: string; content: string; preview?: string }
export const SKETCH_WIDTH = 900
export const SKETCH_HEIGHT = 500
export const escapeXml = (text: string): string => text.replace(/[<>&"']/g, c => ({ '<': '&lt;', '>': '&gt;', '&': '&amp;', '"': '&quot;', "'": '&apos;' })[c]!)
export function sketchPath(stroke: SketchStroke): string {
  const first = stroke.points[0]
  const last = stroke.points.at(-1)
  if (!first || !last) return ''
  if (stroke.kind === 'rect') return `M${first.x},${first.y} H${last.x} V${last.y} H${first.x} Z`
  if (stroke.kind === 'line' || stroke.kind === 'arrow') {
    const line = `M${first.x},${first.y} L${last.x},${last.y}`
    if (stroke.kind === 'line') return line
    const angle = Math.atan2(last.y - first.y, last.x - first.x)
    const arm = (offset: number) => `${Math.round(last.x - 16 * Math.cos(angle + offset))},${Math.round(last.y - 16 * Math.sin(angle + offset))}`
    return `${line} M${arm(.5)} L${last.x},${last.y} L${arm(-.5)}`
  }
  return stroke.points.map((p, i) => `${i ? 'L' : 'M'}${p.x},${p.y}`).join(' ') + (stroke.points.length === 1 ? ' l0.1,0.1' : '')
}
export function sketchSvg(strokes: readonly SketchStroke[]): string {
  return `<svg xmlns="http://www.w3.org/2000/svg" width="900" height="500" viewBox="0 0 900 500"><rect width="900" height="500" fill="#fff"/>${strokes.map(s => s.kind === 'text'
    ? `<text x="${s.points[0]?.x ?? 0}" y="${s.points[0]?.y ?? 0}" fill="${escapeXml(s.color)}" font-size="24" font-family="sans-serif">${escapeXml(s.text ?? '')}</text>`
    : `<path d="${sketchPath(s)}" fill="none" stroke="${escapeXml(s.color)}" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"/>`).join('')}</svg>`
}
export function makeSketchAttachment(strokes: readonly SketchStroke[], name: string): ComposerAttachment {
  const svg = sketchSvg(strokes)
  return { id: crypto.randomUUID(), name, preview: `data:image/svg+xml,${encodeURIComponent(svg)}`, content: name + '\n```svg\n' + svg + '\n```' }
}
