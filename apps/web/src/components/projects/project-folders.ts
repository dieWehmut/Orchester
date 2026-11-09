export interface LocalProject { id: string; name: string; folders: string[] }
export function folderName(path: string): string {
  return path.replace(/[\\/]+$/, '').split(/[\\/]/).pop() || path
}
export function normalizeFolder(path: string): string {
  const value = path.trim()
  return /^(\/|[A-Za-z]:[\\/]|\\\\)/.test(value) && !/[\u0000-\u001f]/.test(value)
    ? (/^[A-Za-z]:[\\/]$/.test(value) ? value : value.replace(/[\\/]+$/, '') || '/') : ''
}
export function sameFolder(left: string, right: string): boolean {
  const canonical = (path: string) => /^[A-Za-z]:|^\\\\/.test(path)
    ? path.replace(/\\/g, '/').toLowerCase() : path
  return canonical(left) === canonical(right)
}
