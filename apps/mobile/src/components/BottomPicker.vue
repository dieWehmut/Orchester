<script lang="ts">
export type PickerOption = {
  value: string;
  label: string;
  description?: string;
  status?: string;
  tone?: "primary" | "secondary" | "tertiary";
};

// A history traversal is asynchronous. Wait for it before opening another sheet.
let returningToPage = false;
function returnToPage() {
  returningToPage = true;
  window.addEventListener(
    "popstate",
    () => {
      returningToPage = false;
    },
    { once: true },
  );
  window.history.back();
}
</script>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, useId, watch } from "vue";
import AppIcon from "./AppIcon.vue";

const props = withDefaults(
  defineProps<{
    modelValue: string;
    label: string;
    title: string;
    description?: string;
    placeholder?: string;
    icon?: string;
    inline?: boolean;
    disabled?: boolean;
    options: readonly PickerOption[];
  }>(),
  { placeholder: "请选择", icon: "computer" },
);
const emit = defineEmits<{ "update:modelValue": [value: string] }>();
const id = `picker-${useId()}`;
const trigger = ref<HTMLButtonElement>();
const dialog = ref<HTMLDialogElement>();
const opened = ref(false);
const active = ref(0);
const selected = computed(() =>
  props.options.find((option) => option.value === props.modelValue),
);
let previousOverflow = "";
const backdropPressed = ref(false);

async function open() {
  if (
    opened.value ||
    returningToPage ||
    props.disabled ||
    !props.options.length ||
    !dialog.value
  )
    return;
  const index = props.options.findIndex(
    (option) => option.value === props.modelValue,
  );
  active.value = Math.max(0, index);
  window.history.pushState(
    { ...window.history.state, orchesterPicker: id },
    "",
  );
  window.addEventListener("popstate", onBack);
  previousOverflow = document.body.style.overflow;
  document.body.style.overflow = "hidden";
  opened.value = true;
  dialog.value.showModal();
  await nextTick();
  if (opened.value) focusOption(active.value);
}

function close(fromHistory = false) {
  if (!opened.value) return;
  opened.value = false;
  dialog.value?.close();
  document.body.style.overflow = previousOverflow;
  window.removeEventListener("popstate", onBack);
  trigger.value?.focus({ preventScroll: true });
  if (!fromHistory && window.history.state?.orchesterPicker === id)
    returnToPage();
}

function onBack() {
  close(true);
}

function backdropClick(event: MouseEvent) {
  if (backdropPressed.value && event.target === event.currentTarget) close();
}

function containFocus(event: KeyboardEvent) {
  if (event.key !== "Tab") return;
  const buttons = dialog.value?.querySelectorAll<HTMLButtonElement>(
    'button:not(:disabled):not([tabindex="-1"])',
  );
  const first = buttons?.[0];
  const last = buttons?.[buttons.length - 1];
  if (event.shiftKey && document.activeElement === first) {
    event.preventDefault();
    last?.focus();
  } else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault();
    first?.focus();
  }
}

function choose(option: PickerOption) {
  if (props.disabled) return;
  close();
  emit("update:modelValue", option.value);
}

function focusOption(index: number) {
  const option =
    dialog.value?.querySelectorAll<HTMLButtonElement>("[role=option]")[index];
  option?.focus({ preventScroll: true });
  option?.scrollIntoView({ block: "nearest" });
}

function move(event: KeyboardEvent) {
  const last = props.options.length - 1;
  let index = active.value;
  if (event.key === "ArrowDown") index = Math.min(last, index + 1);
  else if (event.key === "ArrowUp") index = Math.max(0, index - 1);
  else if (event.key === "Home") index = 0;
  else if (event.key === "End") index = last;
  else return;
  event.preventDefault();
  active.value = index;
  focusOption(index);
}

watch(
  () => props.disabled,
  (disabled) => {
    if (disabled) close();
  },
);
watch(
  () => props.options,
  async (options) => {
    if (!opened.value) return;
    if (!options.length) {
      close();
      return;
    }
    active.value = Math.min(active.value, options.length - 1);
    await nextTick();
    if (opened.value && !dialog.value?.contains(document.activeElement))
      focusOption(active.value);
  },
);
onBeforeUnmount(() => close());
</script>

<template>
  <div class="bottom-picker" :class="{ 'picker-inline': inline }">
    <span :id="`${id}-label`" class="picker-label">{{ label }}</span>
    <button
      :id="`${id}-trigger`"
      ref="trigger"
      class="picker-trigger"
      type="button"
      :disabled="disabled || !options.length"
      aria-haspopup="dialog"
      :aria-expanded="opened"
      :aria-controls="`${id}-dialog`"
      :aria-labelledby="`${id}-label ${id}-value`"
      @click="open"
      @keydown.down.prevent="open"
      @keydown.up.prevent="open"
    >
      <AppIcon :name="icon" class="picker-trigger-icon" />
      <span class="picker-copy">
        <span :id="`${id}-value`" class="picker-value">{{
          selected?.label || placeholder
        }}</span>
        <span v-if="selected?.description" class="picker-detail">{{
          selected.description
        }}</span>
        <span
          v-if="selected?.status"
          class="picker-status"
          :data-tone="selected.tone"
          >{{ selected.status }}</span
        >
      </span>
      <AppIcon
        name="chevron"
        class="picker-arrow"
        :class="{ 'picker-arrow-open': opened }"
      />
    </button>
    <Teleport to="body">
      <dialog
        :id="`${id}-dialog`"
        ref="dialog"
        class="picker-overlay"
        :aria-labelledby="`${id}-title`"
        v-bind="description ? { 'aria-describedby': `${id}-description` } : {}"
        @cancel.prevent="close()"
        @close="close()"
        @keydown="containFocus"
        @pointerdown="backdropPressed = $event.target === $event.currentTarget"
        @click="backdropClick"
      >
        <section class="picker-sheet">
          <div class="picker-handle" aria-hidden="true" />
          <header class="picker-header">
            <div class="picker-copy">
              <h2 :id="`${id}-title`">{{ title }}</h2>
              <p
                v-if="description"
                :id="`${id}-description`"
                class="picker-sheet-detail"
              >
                {{ description }}
              </p>
            </div>
            <button
              class="picker-close"
              type="button"
              :aria-label="`关闭${title}`"
              @click="close()"
            >
              ×
            </button>
          </header>
          <div
            class="picker-options"
            role="listbox"
            :aria-labelledby="`${id}-title`"
            @keydown="move"
          >
            <button
              v-for="(option, index) in options"
              :key="option.value"
              type="button"
              class="picker-option"
              role="option"
              :aria-selected="modelValue === option.value"
              :tabindex="active === index ? 0 : -1"
              @focus="active = index"
              @click="choose(option)"
            >
              <span class="picker-copy">
                <span class="picker-option-label" :title="option.label">{{
                  option.label
                }}</span>
                <span v-if="option.description" class="picker-detail">{{
                  option.description
                }}</span>
                <span
                  v-if="option.status"
                  class="picker-status"
                  :data-tone="option.tone"
                  >{{ option.status }}</span
                >
              </span>
              <span class="picker-selection" aria-hidden="true">
                <AppIcon v-if="modelValue === option.value" name="check" />
              </span>
            </button>
          </div>
        </section>
      </dialog>
    </Teleport>
  </div>
</template>

<style scoped>
.bottom-picker {
  display: grid;
  gap: 8px;
  min-width: 0;
}
.picker-inline {
  display: flex;
  align-items: center;
  gap: 12px;
}
.picker-label {
  color: var(--muted);
  font-size: 13px;
  flex-shrink: 0;
}
.picker-trigger {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  min-width: 0;
  min-height: 56px;
  padding: 12px 14px;
  text-align: left;
  border: 1px solid var(--line);
  border-radius: 16px;
  background: var(--surface);
}
.picker-trigger[aria-expanded="true"] {
  border-color: var(--primary);
}
.picker-trigger-icon {
  color: var(--primary);
  width: 20px;
  height: 20px;
}
.picker-copy {
  display: block;
  flex: 1;
  min-width: 0;
}
.picker-value {
  display: -webkit-box;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
  overflow: hidden;
  overflow-wrap: anywhere;
  color: var(--text);
  font-size: 14px;
  line-height: 1.55;
}
.picker-detail {
  display: block;
  margin-top: 4px;
  font-size: 12px;
  line-height: 1.55;
  color: var(--muted);
  overflow-wrap: anywhere;
}
.picker-arrow {
  width: 18px;
  height: 18px;
  color: var(--muted);
  transform: rotate(90deg);
}
.picker-arrow-open {
  transform: rotate(-90deg);
}
.picker-overlay {
  position: fixed;
  inset: 0;
  width: 100%;
  max-width: none;
  height: 100dvh;
  max-height: none;
  margin: 0;
  padding: env(safe-area-inset-top) 0 0;
  border: 0;
  background: transparent;
  color: var(--text);
  overflow: hidden;
}
.picker-overlay[open] {
  display: flex;
  align-items: flex-end;
  justify-content: center;
}
.picker-overlay::backdrop {
  background: color-mix(in srgb, var(--bg) 76%, transparent);
  backdrop-filter: blur(5px);
}
.picker-sheet {
  display: flex;
  flex-direction: column;
  width: min(100%, 640px);
  max-height: min(82dvh, calc(100dvh - env(safe-area-inset-top) - 20px));
  padding: 12px max(20px, env(safe-area-inset-right))
    calc(20px + env(safe-area-inset-bottom))
    max(20px, env(safe-area-inset-left));
  border: 1px solid var(--line);
  border-bottom: 0;
  border-radius: 30px 30px 0 0;
  background: var(--surface);
  overflow: hidden;
}
.picker-overlay[open] .picker-sheet {
  animation: picker-enter 180ms ease-out;
}
@keyframes picker-enter {
  from {
    transform: translateY(20px);
    opacity: 0;
  }
  to {
    transform: translateY(0);
    opacity: 1;
  }
}
@media (prefers-reduced-motion: reduce) {
  .picker-overlay[open] .picker-sheet {
    animation: none;
  }
}
.picker-handle {
  flex-shrink: 0;
  width: 36px;
  height: 4px;
  margin: 0 auto 14px;
  border-radius: 4px;
  background: var(--line);
}
.picker-header {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-shrink: 0;
  margin-bottom: 16px;
}
.picker-header h2 {
  font-size: 20px;
  color: var(--text);
}
.picker-sheet-detail {
  margin-top: 6px;
  color: var(--muted);
  font-size: 13px;
  line-height: 1.6;
  overflow-wrap: anywhere;
}
.picker-close {
  flex-shrink: 0;
  width: 48px;
  height: 48px;
  border-radius: 50%;
  background: var(--raised);
  color: var(--text);
  font-size: 26px;
}
.picker-options {
  display: grid;
  grid-auto-rows: max-content;
  align-content: start;
  gap: 8px;
  min-height: 0;
  padding: 4px;
  overflow-y: auto;
  overscroll-behavior: contain;
  scrollbar-color: var(--line) var(--surface);
}
.picker-option {
  display: flex;
  align-items: center;
  gap: 12px;
  min-width: 0;
  min-height: 64px;
  padding: 14px;
  border: 1px solid transparent;
  border-radius: 18px;
  text-align: left;
}
.picker-option[aria-selected="true"] {
  border-color: var(--primary);
  background: var(--raised);
}
.picker-option:focus-visible {
  outline-offset: -3px;
}
.picker-option:active {
  background: var(--raised);
}
.picker-option-label {
  display: -webkit-box;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 3;
  overflow: hidden;
  font-size: 15px;
  line-height: 1.6;
  overflow-wrap: anywhere;
}
.picker-status {
  display: inline-block;
  margin-top: 8px;
  color: var(--muted);
  font-size: 12px;
  line-height: 1.5;
}
.picker-status[data-tone="primary"] {
  color: var(--primary);
}
.picker-status[data-tone="secondary"] {
  color: var(--secondary);
}
.picker-status[data-tone="tertiary"] {
  color: var(--tertiary);
}
.picker-selection {
  display: grid;
  place-items: center;
  width: 24px;
  height: 24px;
  flex-shrink: 0;
  border: 1px solid var(--line);
  border-radius: 50%;
}
.picker-option[aria-selected="true"] .picker-selection {
  background: var(--primary);
  border-color: var(--primary);
  color: var(--on-primary);
}
.picker-selection svg {
  width: 16px;
  height: 16px;
}
</style>
