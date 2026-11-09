<script setup lang="ts">
defineProps<{ name: string }>();
const paths: Record<string, string> = {
  folder: "M3 6h7l2 2h9v12H3z",
  file: "M5 3h9l5 5v13H5z M14 3v6h5 M9 13h6 M9 17h6",
  settings: "M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8 M9 3h6l1 3 3 1 2 5-2 5-3 1-1 3H9l-1-3-3-1-2-5 2-5 3-1z",
  dashboard: "M3 3h7v7H3z M14 3h7v7h-7z M3 14h7v7H3z M14 14h7v7h-7z",
  task: "M12 3v18 M3 12h18",
  history: "M3 11a9 9 0 1 1 2 7 M3 4v7h7 M12 7v5l3 2",
  more: "M4 12h.01 M12 12h.01 M20 12h.01",
  computer: "M3 4h18v12H3z M8 21h8 M12 16v5",
  palette:
    "M12 3a9 9 0 1 0 0 18h1a2 2 0 0 0 1-4 2 2 0 0 1 1-4h3a3 3 0 0 0 3-3c0-4-4-7-9-7 M7 9h.01 M10 6h.01 M15 6h.01 M18 9h.01",
  info: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18 M12 11v6 M12 7h.01",
  back: "M19 12H5 M12 5l-7 7 7 7",
  chevron: "M9 5l7 7-7 7",
  check: "M5 12l4 4L19 6",
  refresh: "M20 7a9 9 0 1 0 1 8 M20 3v5h-5",
  send: "M21 3l-7 18-4-7-7-4z M10 14L21 3",
  stop: "M6 6h12v12H6z",
  agent:
    "M9 3h6 M12 3v3 M5 6h14v14H5z M9 11h.01 M15 11h.01 M9 16h6 M2 10v6 M22 10v6",
};
</script>
<template>
  <svg
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.8"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
    focusable="false"
  >
    <path :d="paths[name] || paths.info!" />
  </svg>
</template>
