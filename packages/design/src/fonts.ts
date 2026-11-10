import { readStored } from './storage'
import {
  CONTENT_FONT_STORAGE_KEY,
  CONTENT_FONT_WEIGHT_STORAGE_KEY,
  DEFAULT_CONTENT_FONT,
  DEFAULT_FONT_WEIGHT,
  DEFAULT_UI_FONT,
  UI_FONT_STORAGE_KEY,
  UI_FONT_WEIGHT_STORAGE_KEY,
  applyContentFontToDocument,
  applyContentFontWeightToDocument,
  applyFontWeightToDocument,
  applyUiFontToDocument,
  isContentFont,
  isFontWeight,
  isUiFont,
  readDocumentContentFont,
  readDocumentContentFontWeight,
  readDocumentFontWeight,
  readDocumentUiFont,
} from './theme'

/** Restore only typography, so native surfaces can keep their own palettes. */
export function initFontAppearance() {
  const storedUiFont = readStored(UI_FONT_STORAGE_KEY)
  const storedContentFont = readStored(CONTENT_FONT_STORAGE_KEY)
  const storedUiFontWeight = readStored(UI_FONT_WEIGHT_STORAGE_KEY)
  const storedContentFontWeight = readStored(CONTENT_FONT_WEIGHT_STORAGE_KEY)
  const fonts = {
    uiFont: isUiFont(storedUiFont)
      ? storedUiFont : (readDocumentUiFont() ?? DEFAULT_UI_FONT),
    contentFont: isContentFont(storedContentFont)
      ? storedContentFont : (readDocumentContentFont() ?? DEFAULT_CONTENT_FONT),
    uiFontWeight: isFontWeight(storedUiFontWeight)
      ? storedUiFontWeight : (readDocumentFontWeight() ?? DEFAULT_FONT_WEIGHT),
    contentFontWeight: isFontWeight(storedContentFontWeight)
      ? storedContentFontWeight : (readDocumentContentFontWeight() ?? DEFAULT_FONT_WEIGHT),
  }
  applyUiFontToDocument(fonts.uiFont)
  applyContentFontToDocument(fonts.contentFont)
  applyFontWeightToDocument(fonts.uiFontWeight)
  applyContentFontWeightToDocument(fonts.contentFontWeight)
  return fonts
}
