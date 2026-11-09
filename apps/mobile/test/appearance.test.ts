import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { useAppearance } from "../src/composables/use-appearance";

let stored: Map<string, string>;
let colors: Map<string, string>;
let attributes: Map<string, string>;

beforeEach(() => {
  stored = new Map();
  colors = new Map();
  attributes = new Map();
  vi.stubGlobal("localStorage", {
    getItem: (key: string) => stored.get(key) ?? null,
    setItem: (key: string, value: string) => stored.set(key, value),
  });
  vi.stubGlobal("document", {
    documentElement: {
      style: {
        setProperty: (key: string, value: string) => colors.set(key, value),
      },
      setAttribute: (key: string, value: string) => attributes.set(key, value),
    },
  });
});

afterEach(() => vi.unstubAllGlobals());

it("defaults to coral when no preference has been saved", () => {
  expect(useAppearance().theme.value).toBe("coral");
});

it("restores the selected theme after reinitializing the app", () => {
  const appearance = useAppearance();
  const originalBackground = colors.get("--bg");
  appearance.setTheme("sea");

  expect(appearance.theme.value).toBe("sea");
  expect(colors.get("--bg")).not.toBe(originalBackground);
  expect(attributes.get("data-theme")).toBe("sea");
  expect([...stored.entries()]).toEqual([["orchester.theme", "sea"]]);
  expect(useAppearance().theme.value).toBe("sea");
});

it.each(["unknown", "__proto__", '{"theme":"rose"}'])(
  "falls back to coral for an invalid saved preference: %s",
  (invalid) => {
    stored.set("orchester.theme", invalid);
    expect(useAppearance().theme.value).toBe("coral");
    expect(attributes.get("data-theme")).toBe("coral");
  },
);

it("keeps theme selection working when preference storage throws", () => {
  vi.stubGlobal("localStorage", {
    getItem: () => {
      throw new Error("Storage blocked");
    },
    setItem: () => {
      throw new Error("Storage blocked");
    },
  });
  const appearance = useAppearance();
  expect(appearance.theme.value).toBe("coral");

  expect(() => appearance.setTheme("rose")).not.toThrow();
  expect(appearance.theme.value).toBe("rose");
  expect(attributes.get("data-theme")).toBe("rose");
});

it("ignores an invalid selection instead of persisting it", () => {
  const appearance = useAppearance();
  appearance.setTheme("forest");
  const background = colors.get("--bg");
  appearance.setTheme("unknown");

  expect(appearance.theme.value).toBe("forest");
  expect(colors.get("--bg")).toBe(background);
  expect(stored.get("orchester.theme")).toBe("forest");
});

it("can initialize without browser storage or a document", () => {
  vi.stubGlobal("localStorage", undefined);
  vi.stubGlobal("document", undefined);
  const appearance = useAppearance();
  expect(appearance.theme.value).toBe("coral");
  expect(() => appearance.setTheme("indigo")).not.toThrow();
  expect(appearance.theme.value).toBe("indigo");
});
