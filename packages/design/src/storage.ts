/**
 * A `localStorage` wrapper that cannot throw.
 *
 * Storage access is not merely absent in some contexts, it *throws*: Safari in
 * private mode, a sandboxed iframe without `allow-same-origin`, and a browser
 * with site data blocked all raise a `SecurityError` on the first property
 * access. The project site embeds a third-party comment widget in an iframe, so
 * this is a case we will actually meet, and losing the theme preference is
 * acceptable where a blank page is not.
 */

export function readStored(key: string): string | null {
  try {
    return globalThis.localStorage?.getItem(key) ?? null
  } catch {
    return null
  }
}

export function writeStored(key: string, value: string): void {
  try {
    globalThis.localStorage?.setItem(key, value)
  } catch {
    /* A lost preference is not worth a broken page. */
  }
}

export function clearStored(key: string): void {
  try {
    globalThis.localStorage?.removeItem(key)
  } catch {
    /* As above. */
  }
}
