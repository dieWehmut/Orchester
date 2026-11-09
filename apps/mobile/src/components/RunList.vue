<script setup lang="ts">
import type { Run } from "../client";
import { runState } from "../presentation";
import AppIcon from "./AppIcon.vue";
defineProps<{ runs: Run[]; disabled?: boolean }>();
defineEmits<{ open: [run: Run] }>();
</script>
<template>
  <div class="list-panel">
    <button
      v-for="run in runs"
      :key="run.id"
      class="list-row run-row"
      :disabled="disabled"
      @click="$emit('open', run)"
    >
      <span class="icon-disc secondary"><AppIcon name="agent" /></span
      ><span class="row-copy"
        ><strong>{{ run.prompt }}</strong
        ><small>{{ run.agent }}</small></span
      ><span class="run-status" :data-state="run.state">{{
        runState(run.state)
      }}</span
      ><AppIcon class="chevron" name="chevron" />
    </button>
  </div>
</template>
