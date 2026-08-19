import { copyFile } from 'node:fs/promises'
import { join } from 'node:path'

export async function copyIndexTo404(outDir: string): Promise<void> {
  await copyFile(join(outDir, 'index.html'), join(outDir, '404.html'))
}
