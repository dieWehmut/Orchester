// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { createApp, h, nextTick, reactive, type App } from "vue";
import BottomPicker, {
  type PickerOption,
} from "../src/components/BottomPicker.vue";

const options: PickerOption[] = [
  { value: "all", label: "全部设备", description: "2 台电脑" },
  {
    value: "first",
    label: "工作电脑",
    description: "http://192.168.1.10:7337",
    status: "已连接",
    tone: "secondary",
  },
  {
    value: "second",
    label: "个人电脑",
    description: "http://192.168.1.20:7337",
    status: "需要配对",
  },
];
let apps: App[];
let currentState: Record<string, unknown>;
let stack: Record<string, unknown>[];

beforeEach(() => {
  apps = [];
  currentState = { page: "projects" };
  stack = [];
  vi.spyOn(window.history, "state", "get").mockImplementation(
    () => currentState,
  );
  vi.spyOn(window.history, "pushState").mockImplementation((state) => {
    stack.push(currentState);
    currentState = state;
  });
  vi.spyOn(window.history, "back").mockImplementation(() => {
    currentState = stack.pop() ?? { page: "projects" };
    queueMicrotask(() =>
      window.dispatchEvent(
        new PopStateEvent("popstate", { state: currentState }),
      ),
    );
  });
  // jsdom does not implement dialog or layout. Browser QA covers native focus containment.
  HTMLDialogElement.prototype.showModal = function () {
    this.open = true;
  };
  HTMLDialogElement.prototype.close = function () {
    this.open = false;
    this.dispatchEvent(new Event("close"));
  };
  HTMLElement.prototype.scrollIntoView = vi.fn();
});

afterEach(async () => {
  apps.forEach((app) => app.unmount());
  await nextTick();
  document.body.replaceChildren();
  document.body.style.overflow = "";
  vi.restoreAllMocks();
});

function mountPicker(value = "first", withSibling = false) {
  const model = reactive({ value, disabled: false, options: [...options] });
  const changed = vi.fn((value: string) => {
    model.value = value;
  });
  const root = document.createElement("div");
  document.body.append(root);
  const renderPicker = (value = model.value) =>
    h(BottomPicker, {
      modelValue: value,
      label: "设备",
      title: "选择设备",
      description: "选择要浏览项目的电脑",
      options: model.options,
      disabled: model.disabled,
      "onUpdate:modelValue": changed,
    });
  const app = createApp({
    render: () =>
      withSibling
        ? h("div", [renderPicker(), renderPicker("second")])
        : renderPicker(),
  });
  app.mount(root);
  apps.push(app);
  const trigger = root.querySelector<HTMLButtonElement>(".picker-trigger")!;
  const dialog = document.getElementById(
    trigger.getAttribute("aria-controls")!,
  ) as HTMLDialogElement;
  return {
    app,
    model,
    changed,
    trigger,
    dialog,
    items: () => [
      ...dialog.querySelectorAll<HTMLButtonElement>("[role=option]"),
    ],
  };
}

async function openPicker(picker: ReturnType<typeof mountPicker>) {
  picker.trigger.focus();
  picker.trigger.click();
  await nextTick();
}

it("shows device name, full address and written status with unique dialog labels", () => {
  const first = mountPicker("first", true);
  const secondTrigger =
    document.querySelectorAll<HTMLButtonElement>(".picker-trigger")[1]!;
  const secondDialog = document.getElementById(
    secondTrigger.getAttribute("aria-controls")!,
  )!;
  expect(first.trigger.textContent).toContain("工作电脑");
  expect(first.trigger.textContent).toContain("http://192.168.1.10:7337");
  expect(first.trigger.textContent).toContain("已连接");
  expect(secondTrigger.textContent).toContain("需要配对");
  expect(first.dialog.id).not.toBe(secondDialog.id);
  expect(
    document.getElementById(first.dialog.getAttribute("aria-labelledby")!)
      ?.textContent,
  ).toBe("选择设备");
  expect(first.trigger.getAttribute("aria-haspopup")).toBe("dialog");
});

it("focuses the current option and supports arrow, Home and End navigation before choosing", async () => {
  const picker = mountPicker();
  await openPicker(picker);
  expect(picker.dialog.open).toBe(true);
  expect(picker.trigger.getAttribute("aria-expanded")).toBe("true");
  expect(document.activeElement).toBe(picker.items()[1]);
  expect(picker.items()[1]?.getAttribute("aria-selected")).toBe("true");
  const move = (key: string) =>
    document.activeElement?.dispatchEvent(
      new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true }),
    );
  move("ArrowDown");
  expect(document.activeElement).toBe(picker.items()[2]);
  move("Home");
  expect(document.activeElement).toBe(picker.items()[0]);
  move("End");
  move("ArrowUp");
  expect(document.activeElement).toBe(picker.items()[1]);
  expect(picker.changed).not.toHaveBeenCalled();
  picker.items()[2]!.click();
  await nextTick();
  expect(picker.model.value).toBe("second");
  expect(picker.trigger.textContent).toContain("个人电脑");
  expect(picker.dialog.open).toBe(false);
  expect(document.activeElement).toBe(picker.trigger);
  expect(window.history.state).toEqual({ page: "projects" });
});

it("Escape cancellation restores focus, scroll and history without selecting a different device", async () => {
  document.body.style.overflow = "auto";
  const picker = mountPicker();
  await openPicker(picker);
  expect(document.body.style.overflow).toBe("hidden");
  const event = new Event("cancel", { cancelable: true });
  picker.dialog.dispatchEvent(event);
  await nextTick();
  expect(event.defaultPrevented).toBe(true);
  expect(picker.changed).not.toHaveBeenCalled();
  expect(picker.dialog.open).toBe(false);
  expect(picker.trigger.getAttribute("aria-expanded")).toBe("false");
  expect(document.activeElement).toBe(picker.trigger);
  expect(document.body.style.overflow).toBe("auto");
  expect(window.history.state).toEqual({ page: "projects" });
  expect(window.history.back).toHaveBeenCalledTimes(1);
});

it("Back closes the sheet without issuing a second history traversal", async () => {
  const picker = mountPicker();
  await openPicker(picker);
  window.history.back();
  await nextTick();
  expect(picker.dialog.open).toBe(false);
  expect(picker.changed).not.toHaveBeenCalled();
  expect(window.history.back).toHaveBeenCalledTimes(1);
  expect(document.body.style.overflow).toBe("");
});

it("cycles Tab focus between the active option and the close button", async () => {
  const picker = mountPicker();
  await openPicker(picker);
  const close =
    picker.dialog.querySelector<HTMLButtonElement>(".picker-close")!;
  picker
    .items()[1]!
    .dispatchEvent(
      new KeyboardEvent("keydown", {
        key: "Tab",
        bubbles: true,
        cancelable: true,
      }),
    );
  expect(document.activeElement).toBe(close);
  close.dispatchEvent(
    new KeyboardEvent("keydown", {
      key: "Tab",
      shiftKey: true,
      bubbles: true,
      cancelable: true,
    }),
  );
  expect(document.activeElement).toBe(picker.items()[1]);
});

it("dismisses only when the pointer starts and ends on the backdrop", async () => {
  const picker = mountPicker();
  await openPicker(picker);
  picker.dialog
    .querySelector(".picker-sheet")!
    .dispatchEvent(new Event("pointerdown", { bubbles: true }));
  picker.dialog.click();
  expect(picker.dialog.open).toBe(true);
  picker.dialog.dispatchEvent(new Event("pointerdown", { bubbles: true }));
  picker.dialog.click();
  await nextTick();
  expect(picker.dialog.open).toBe(false);
  expect(picker.changed).not.toHaveBeenCalled();
});

it("closes on disconnect or an empty option list and keeps the disabled trigger inactive", async () => {
  const picker = mountPicker();
  await openPicker(picker);
  picker.model.disabled = true;
  await nextTick();
  expect(picker.dialog.open).toBe(false);
  picker.trigger.click();
  await nextTick();
  expect(picker.dialog.open).toBe(false);
  picker.model.disabled = false;
  await nextTick();
  await openPicker(picker);
  expect(picker.dialog.open).toBe(true);
  picker.model.options = [];
  await nextTick();
  expect(picker.dialog.open).toBe(false);
  expect(picker.trigger.disabled).toBe(true);
  expect(picker.changed).not.toHaveBeenCalled();
});

it("cleans up a visible sheet when its workbench or page unmounts", async () => {
  const picker = mountPicker();
  await openPicker(picker);
  picker.app.unmount();
  apps = [];
  await nextTick();
  expect(document.querySelector("dialog")).toBeNull();
  expect(document.body.style.overflow).toBe("");
  expect(window.history.state).toEqual({ page: "projects" });
  const another = mountPicker("all");
  await openPicker(another);
  expect(another.dialog.open).toBe(true);
  expect(another.items()[0]?.getAttribute("aria-selected")).toBe("true");
});
