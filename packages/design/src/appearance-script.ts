// Keep this bootstrap module dependency-free: Vite loads it while evaluating
// an app config in Node, before the source-only package graph is bundled.
// These values intentionally mirror theme.ts; the appearance tests assert the
// public storage keys and defaults so the axes cannot drift silently.
const THEME_MODES = ['light', 'dark'] as const
const COLOR_SCHEMES = ['codex', 'violet', 'teal', 'rose'] as const
const INTENSITIES = ['calm', 'vivid'] as const
const SURFACES = ['web', 'site', 'desktop'] as const
const UI_FONTS = ['system', 'sans', 'serif'] as const
const CONTENT_FONTS = ['ui', 'mono'] as const
const RAIL_APPEARANCES = ['solid', 'translucent'] as const
const FONT_WEIGHTS = ['regular', 'medium'] as const
const DEFAULT_THEME = 'dark'
const DEFAULT_COLOR_SCHEME = 'rose'
const DEFAULT_INTENSITY = 'vivid'
const DEFAULT_SURFACE = 'web'
const DEFAULT_UI_FONT = 'system'
const DEFAULT_CONTENT_FONT = 'mono'
const DEFAULT_RAIL_APPEARANCE = 'solid'
const DEFAULT_FONT_WEIGHT = 'regular'
const THEME_ATTRIBUTE = 'data-theme'
const COLOR_SCHEME_ATTRIBUTE = 'data-color-scheme'
const INTENSITY_ATTRIBUTE = 'data-intensity'
const REDUCED_MOTION_ATTRIBUTE = 'data-reduced-motion'
const SURFACE_ATTRIBUTE = 'data-orchester-surface'
const UI_FONT_ATTRIBUTE = 'data-ui-font'
const CONTENT_FONT_ATTRIBUTE = 'data-content-font'
const RAIL_APPEARANCE_ATTRIBUTE = 'data-rail-appearance'
const UI_FONT_WEIGHT_ATTRIBUTE = 'data-ui-font-weight'
const CONTENT_FONT_WEIGHT_ATTRIBUTE = 'data-content-font-weight'
const OS_ATTRIBUTE = 'data-orchester-os'
const THEME_STORAGE_KEY = 'orchester:theme'
const COLOR_SCHEME_STORAGE_KEY = 'orchester:color-scheme'
const INTENSITY_STORAGE_KEY = 'orchester:intensity'
const REDUCED_MOTION_STORAGE_KEY = 'orchester:reduced-motion'
const UI_FONT_STORAGE_KEY = 'orchester:ui-font'
const CONTENT_FONT_STORAGE_KEY = 'orchester:content-font'
const RAIL_APPEARANCE_STORAGE_KEY = 'orchester:rail-appearance'
const UI_FONT_WEIGHT_STORAGE_KEY = 'orchester:ui-font-weight'
const CONTENT_FONT_WEIGHT_STORAGE_KEY = 'orchester:content-font-weight'

/**
 * Inline this script in `<head>` before loading application styles.
 *
 * It deliberately contains no imports at runtime: both the local WebUI and the
 * project site can execute it before their JavaScript bundles are available, and
 * the desktop shell can execute it before either.
 *
 * Every axis it writes exists to prevent a flash of the wrong thing — not just
 * the wrong theme, but the wrong roundness of accent, the wrong motion budget,
 * and the wrong window chrome for the platform.
 */
export const APPEARANCE_BOOTSTRAP_SCRIPT = `(() => {
  const root = document.documentElement;
  const themes = ${JSON.stringify(THEME_MODES)};
  const preferences = ${JSON.stringify(['system', 'light', 'dark'])};
  const schemes = ${JSON.stringify(COLOR_SCHEMES)};
  const intensities = ${JSON.stringify(INTENSITIES)};
  const surfaces = ${JSON.stringify(SURFACES)};
  const uiFonts = ${JSON.stringify(UI_FONTS)};
  const contentFonts = ${JSON.stringify(CONTENT_FONTS)};
  const railAppearances = ${JSON.stringify(RAIL_APPEARANCES)};
  const fontWeights = ${JSON.stringify(FONT_WEIGHTS)};
  let stored = {};
  try {
    const storage = globalThis.localStorage;
    if (storage) {
      stored = {
        theme: storage.getItem(${JSON.stringify(THEME_STORAGE_KEY)}),
        scheme: storage.getItem(${JSON.stringify(COLOR_SCHEME_STORAGE_KEY)}),
        intensity: storage.getItem(${JSON.stringify(INTENSITY_STORAGE_KEY)}),
        reducedMotion: storage.getItem(${JSON.stringify(REDUCED_MOTION_STORAGE_KEY)}),
        uiFont: storage.getItem(${JSON.stringify(UI_FONT_STORAGE_KEY)}),
        contentFont: storage.getItem(${JSON.stringify(CONTENT_FONT_STORAGE_KEY)}),
        railAppearance: storage.getItem(${JSON.stringify(RAIL_APPEARANCE_STORAGE_KEY)}),
        uiFontWeight: storage.getItem(${JSON.stringify(UI_FONT_WEIGHT_STORAGE_KEY)}),
        contentFontWeight: storage.getItem(${JSON.stringify(CONTENT_FONT_WEIGHT_STORAGE_KEY)}),
      };
    }
  } catch {}

  const pick = (allowed, ...candidates) => {
    for (const candidate of candidates) {
      if (typeof candidate === 'string' && allowed.includes(candidate)) return candidate;
    }
    return null;
  };

  const systemTheme = typeof globalThis.matchMedia === 'function'
    && globalThis.matchMedia('(prefers-color-scheme: light)').matches
      ? 'light'
      : ${JSON.stringify(DEFAULT_THEME)};

  // "system" is stored as a preference but must resolve to a real theme here,
  // because the data-theme attribute is what the stylesheet matches on.
  const preference = pick(preferences, stored.theme) || 'system';
  const theme = preference === 'system'
    ? systemTheme
    : preference || pick(themes, root.getAttribute(${JSON.stringify(THEME_ATTRIBUTE)}), systemTheme)
      || ${JSON.stringify(DEFAULT_THEME)};
  const scheme = pick(schemes, stored.scheme, root.getAttribute(${JSON.stringify(COLOR_SCHEME_ATTRIBUTE)}))
    || ${JSON.stringify(DEFAULT_COLOR_SCHEME)};
  const intensity = pick(intensities, stored.intensity, root.getAttribute(${JSON.stringify(INTENSITY_ATTRIBUTE)}))
    || ${JSON.stringify(DEFAULT_INTENSITY)};
  // The host may have announced the surface before this script ran; a value it
  // set outranks the default, because only the host knows how it was packaged.
  const surface = pick(surfaces, root.getAttribute(${JSON.stringify(SURFACE_ATTRIBUTE)}), stored.surface)
    || ${JSON.stringify(DEFAULT_SURFACE)};
  const uiFont = pick(uiFonts, stored.uiFont, root.getAttribute(${JSON.stringify(UI_FONT_ATTRIBUTE)}))
    || ${JSON.stringify(DEFAULT_UI_FONT)};
  const contentFont = pick(contentFonts, stored.contentFont, root.getAttribute(${JSON.stringify(CONTENT_FONT_ATTRIBUTE)}))
    || ${JSON.stringify(DEFAULT_CONTENT_FONT)};
  const railAppearance = pick(railAppearances, stored.railAppearance, root.getAttribute(${JSON.stringify(RAIL_APPEARANCE_ATTRIBUTE)}))
    || ${JSON.stringify(DEFAULT_RAIL_APPEARANCE)};
  const uiFontWeight = pick(fontWeights, stored.uiFontWeight, root.getAttribute(${JSON.stringify(UI_FONT_WEIGHT_ATTRIBUTE)}))
    || ${JSON.stringify(DEFAULT_FONT_WEIGHT)};
  const contentFontWeight = pick(fontWeights, stored.contentFontWeight, root.getAttribute(${JSON.stringify(CONTENT_FONT_WEIGHT_ATTRIBUTE)}))
    || ${JSON.stringify(DEFAULT_FONT_WEIGHT)};

  root.setAttribute(${JSON.stringify(THEME_ATTRIBUTE)}, theme);
  root.setAttribute(${JSON.stringify(COLOR_SCHEME_ATTRIBUTE)}, scheme);
  root.setAttribute(${JSON.stringify(INTENSITY_ATTRIBUTE)}, intensity);
  root.setAttribute(${JSON.stringify(SURFACE_ATTRIBUTE)}, surface);
  root.setAttribute(${JSON.stringify(UI_FONT_ATTRIBUTE)}, uiFont);
  root.setAttribute(${JSON.stringify(CONTENT_FONT_ATTRIBUTE)}, contentFont);
  root.setAttribute(${JSON.stringify(RAIL_APPEARANCE_ATTRIBUTE)}, railAppearance);
  root.setAttribute(${JSON.stringify(UI_FONT_WEIGHT_ATTRIBUTE)}, uiFontWeight);
  root.setAttribute(${JSON.stringify(CONTENT_FONT_WEIGHT_ATTRIBUTE)}, contentFontWeight);
  root.style.colorScheme = theme;

  // Reduced motion has three states. Write nothing when the user has no
  // opinion, so the stylesheet's media query can act.
  if (stored.reducedMotion === 'true' || stored.reducedMotion === 'false') {
    root.setAttribute(${JSON.stringify(REDUCED_MOTION_ATTRIBUTE)}, stored.reducedMotion);
  } else {
    root.removeAttribute(${JSON.stringify(REDUCED_MOTION_ATTRIBUTE)});
  }

  const agent = (typeof globalThis.navigator === 'object' && globalThis.navigator
    && typeof globalThis.navigator.userAgent === 'string')
      ? globalThis.navigator.userAgent.toLowerCase()
      : '';
  const platform = agent.includes('mac os') || agent.includes('macintosh')
    ? 'macos'
    : agent.includes('windows')
      ? 'windows'
      : 'linux';
  root.setAttribute(${JSON.stringify(OS_ATTRIBUTE)}, platform);
})()`