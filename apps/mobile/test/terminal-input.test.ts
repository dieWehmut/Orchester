// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { Terminal } from "@xterm/xterm";

let terminals: Terminal[];
let TerminalClass: typeof import("@xterm/xterm").Terminal;
beforeEach(async () => {
  terminals = [];
  vi.useFakeTimers();
  vi.stubGlobal("ResizeObserver", class {
    observe() {}
    disconnect() {}
    unobserve() {}
  });
  vi.stubGlobal("matchMedia", vi.fn(() => ({
    matches: false,
    addListener() {},
    removeListener() {},
    addEventListener() {},
    removeEventListener() {},
  })));
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue({
    measureText: () => ({ width: 8 }),
  } as unknown as CanvasRenderingContext2D);
  TerminalClass = (await import("@xterm/xterm")).Terminal;
});
afterEach(() => {
  terminals.forEach((terminal) => terminal.dispose());
  document.body.replaceChildren();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

function fixture(screenReaderMode = false) {
  const root = document.createElement("div");
  document.body.append(root);
  const terminal = new TerminalClass({ screenReaderMode });
  terminals.push(terminal);
  terminal.open(root);
  const data: string[] = [];
  terminal.onData((chunk) => data.push(chunk));
  return { terminal, input: terminal.textarea!, data };
}

function androidInput(input: HTMLTextAreaElement, text: string) {
  input.dispatchEvent(new KeyboardEvent("keydown", {
    key: "Unidentified", keyCode: 229, bubbles: true, cancelable: true,
  }));
  input.dispatchEvent(new InputEvent("beforeinput", {
    data: text, inputType: "insertText", bubbles: true, composed: true,
  }));
  input.value += text;
  input.dispatchEvent(new InputEvent("input", {
    data: text, inputType: "insertText", bubbles: true, composed: true,
  }));
  input.dispatchEvent(new KeyboardEvent("keyup", {
    key: "Unidentified", keyCode: 229, bubbles: true,
  }));
}

it.each([false, true])("preserves rapid Android input exactly once (screen reader: %s)", async (screenReaderMode) => {
  const f = fixture(screenReaderMode);
  for (const character of "pwd") androidInput(f.input, character);
  await vi.advanceTimersByTimeAsync(1);
  expect(f.data.join("")).toBe("pwd");
});

it.each([false, true])("preserves rapid repeated characters and emoji (screen reader: %s)", async (mode) => {
  const f = fixture(mode);
  const value = "bookkeeper111111中文🙂🙂";
  for (const character of value) androidInput(f.input, character);
  await vi.advanceTimersByTimeAsync(1);
  expect(f.data.join("")).toBe(value);
});

it.each([false, true])("handles IME input-before-keydown rollover without losing or resending neighbors (screen reader: %s)", async (mode) => {
  const f = fixture(mode);
  for (const character of "jk") {
    f.input.value += character;
    f.input.dispatchEvent(new InputEvent("input", {
      data: character, inputType: "insertText", composed: true, bubbles: true,
    }));
    f.input.dispatchEvent(new KeyboardEvent("keydown", {
      key: character, keyCode: 229, bubbles: true,
    }));
  }
  await vi.advanceTimersByTimeAsync(30);
  f.input.dispatchEvent(new KeyboardEvent("keyup", { keyCode: 74, bubbles: true }));
  f.input.dispatchEvent(new KeyboardEvent("keyup", { keyCode: 75, bubbles: true }));
  expect(f.data.join("")).toBe("jk");
});

it.each([false, true])("keeps the next IME passthrough character when the prior key remains held beyond the timer window (screen reader: %s)", async (mode) => {
  const f = fixture(mode);
  f.input.value = "j";
  f.input.dispatchEvent(new InputEvent("input", { data: "j", inputType: "insertText", composed: true, bubbles: true }));
  f.input.dispatchEvent(new KeyboardEvent("keydown", { key: "j", keyCode: 229, bubbles: true }));
  await vi.advanceTimersByTimeAsync(30);
  f.input.value = "jk";
  f.input.dispatchEvent(new InputEvent("input", { data: "k", inputType: "insertText", composed: true, bubbles: true }));
  f.input.dispatchEvent(new KeyboardEvent("keydown", { key: "k", keyCode: 229, bubbles: true }));
  await vi.advanceTimersByTimeAsync(30);
  expect(f.data.join("")).toBe("jk");
});

it.each([
  [false, false], [true, false], [false, true], [true, true],
])("accepts input after a released hardware key (screen reader: %s, input-before-keydown: %s)", async (mode, inputBeforeKeydown) => {
  const f = fixture(mode);
  f.input.dispatchEvent(new KeyboardEvent("keydown", {
    key: "a", keyCode: 65, bubbles: true, cancelable: true,
  }));
  f.input.dispatchEvent(new KeyboardEvent("keyup", {
    key: "a", keyCode: 65, bubbles: true,
  }));
  f.input.value += "🙂";
  f.input.dispatchEvent(new InputEvent("input", {
    data: "🙂", inputType: "insertText", composed: true, bubbles: true,
  }));
  if (inputBeforeKeydown) {
    f.input.dispatchEvent(new KeyboardEvent("keydown", { keyCode: 229, bubbles: true }));
    f.input.dispatchEvent(new KeyboardEvent("keyup", { keyCode: 229, bubbles: true }));
  }
  await vi.advanceTimersByTimeAsync(30);
  expect(f.data.join("")).toBe("a🙂");
});

it.each([false, true])("commits IME text once and preserves a neighboring non-composition key (screen reader: %s)", async (mode) => {
  const f = fixture(mode);
  f.input.dispatchEvent(new CompositionEvent("compositionstart", { bubbles: true }));
  f.input.value = "café";
  f.input.dispatchEvent(new CompositionEvent("compositionupdate", { data: "café", bubbles: true }));
  f.input.dispatchEvent(new InputEvent("input", {
    data: "café", inputType: "insertCompositionText", isComposing: true, bubbles: true,
  }));
  await vi.advanceTimersByTimeAsync(1);
  expect(f.data).toEqual([]);
  f.input.dispatchEvent(new CompositionEvent("compositionend", { data: "café", bubbles: true }));
  f.input.dispatchEvent(new InputEvent("input", {
    data: "café", inputType: "insertText", bubbles: true, composed: true,
  }));
  androidInput(f.input, "1");
  await vi.advanceTimersByTimeAsync(1);
  expect(f.data.join("")).toBe("café1");
  androidInput(f.input, "1");
  await vi.advanceTimersByTimeAsync(1);
  expect(f.data.join("")).toBe("café11");
});

it("handles Android backspace mutations and hardware control keys", async () => {
  const f = fixture();
  androidInput(f.input, "a");
  f.input.dispatchEvent(new KeyboardEvent("keydown", { keyCode: 229, bubbles: true }));
  f.input.value = "";
  f.input.dispatchEvent(new InputEvent("input", { inputType: "deleteContentBackward", bubbles: true, composed: true }));
  f.input.dispatchEvent(new KeyboardEvent("keyup", { keyCode: 229, bubbles: true }));
  for (const event of [
    { key: "c", keyCode: 67, ctrlKey: true },
    { key: "Enter", keyCode: 13 },
    { key: "ArrowLeft", keyCode: 37 },
    { key: "Backspace", keyCode: 8 },
  ]) {
    f.input.dispatchEvent(new KeyboardEvent("keydown", { ...event, bubbles: true, cancelable: true }));
    f.input.dispatchEvent(new KeyboardEvent("keyup", { ...event, bubbles: true }));
  }
  await vi.advanceTimersByTimeAsync(1);
  expect(f.data.join("")).toBe("a\u007f\u0003\r\u001b[D\u007f");
});

it.each([false, true])("keeps hardware keypress and textarea input from duplicating each other (screen reader: %s)", async (mode) => {
  const f = fixture(mode);
  f.input.dispatchEvent(new KeyboardEvent("keydown", { key: "A", keyCode: 65, bubbles: true, cancelable: true }));
  f.input.dispatchEvent(new KeyboardEvent("keypress", { key: "A", charCode: 65, keyCode: 65, bubbles: true, cancelable: true }));
  f.input.value = "A";
  f.input.dispatchEvent(new InputEvent("input", { data: "A", inputType: "insertText", bubbles: true, composed: true }));
  f.input.dispatchEvent(new KeyboardEvent("keyup", { key: "A", keyCode: 65, bubbles: true }));
  await vi.advanceTimersByTimeAsync(1);
  expect(f.data.join("")).toBe("A");
});

it("preserves bracketed paste control sequences", async () => {
  const f = fixture();
  f.terminal.write("\u001b[?2004h");
  await vi.advanceTimersByTimeAsync(1);
  const event = new Event("paste", { bubbles: true, cancelable: true });
  Object.defineProperty(event, "clipboardData", { value: { getData: () => "echo 中文\n" } });
  f.input.dispatchEvent(event);
  expect(f.data.join("")).toBe("\u001b[200~echo 中文\r\u001b[201~");
});
