import { parseWorkspaceReview } from '@orchester/protokoll'

/** Keep whole filenames and report omissions instead of rejecting a large repository. */
export function workspaceAttachmentContent(raw: unknown, name: string): string {
  const review = parseWorkspaceReview(raw)
  if (!review) throw new Error('Invalid workspace review')
  const lines = [
    'Workspace context (file names and status, not file contents):',
    `Name: ${JSON.stringify(name.slice(0, 256))}`,
    `Branch: ${JSON.stringify(review.branch?.slice(0, 256) ?? null)}`,
    `Working changes: ${review.changes.length}; branch changes: ${review.branch_changes.length}`,
  ]
  const entries = [
    ...[...review.changes].sort((a, b) => Number(a.kind === 'untracked') - Number(b.kind === 'untracked')).map(c =>
      `Working ${c.kind}: ${JSON.stringify(c.path)}${c.staged ? ' [staged]' : ''}${c.unstaged ? ' [unstaged]' : ''}`),
    ...review.branch_changes.map(path => `Branch: ${JSON.stringify(path)}`),
  ]
  let length = lines.join('\n').length
  let omitted = 0
  for (const entry of entries) {
    if (length + entry.length + 1 > 15800) { omitted++; continue }
    lines.push(entry); length += entry.length + 1
  }
  if (omitted) lines.push(`Omitted ${omitted} additional paths to fit the attachment. Request specific files when needed.`)
  return lines.join('\n')
}
