<script setup lang="ts">
import { AgentToolIcon } from '@orchester/design'
const props = defineProps<{ iconKey: string }>()
</script>

<template>
  <AgentToolIcon :icon-key="props.iconKey" />
</template>
