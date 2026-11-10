/** xterm measures its grid at open(), so resolve/load the shared face first. */
export async function terminalFonts(surface: HTMLElement) {
  const style = getComputedStyle(surface);
  const fontFamily = style.getPropertyValue("--font-mono").trim() || "monospace";
  const fontWeight = Number(style.getPropertyValue("--weight-content")) || 400;
  const fontSize = 13;
  try {
    await surface.ownerDocument.fonts?.load(
      `${fontWeight} ${fontSize}px ${fontFamily}`, "W中文",
    );
    await surface.ownerDocument.fonts?.ready;
  } catch {
    // Missing/failed font files still leave the system fallback usable.
  }
  return { fontFamily, fontWeight, fontSize };
}
