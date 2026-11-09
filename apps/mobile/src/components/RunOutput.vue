<script setup lang="ts">
import { computed } from "vue";
import type { Events, Run } from "../client";
import { runFailure, runState } from "../presentation";
import AppIcon from "./AppIcon.vue";
const props = defineProps<{
  run: Run;
  events: Events["events"];
  busy: boolean;
  disabled: boolean;
  resumable: boolean;
}>();
defineEmits<{ cancel: []; continue: []; remove: [] }>();
const labels: Record<string, string> = {
  session_started: "会话已建立",
  turn_started: "开始处理",
  reasoning: "思考摘要",
  message: "Agent 回复",
  result: "结果",
  tool_call: "工具执行",
  file_change: "文件变更",
  todo_list: "任务进度",
  usage: "用量",
  turn_completed: "本轮结束",
  error: "执行错误",
  stopped: "已停止",
};
const output = computed(() =>
  props.events.map(({ seq, event }) => {
    const item =
      typeof event === "object" && event
        ? (event as Record<string, unknown>)
        : {};
    const type = typeof item.type === "string" ? item.type : "event";
    const message =
      typeof item.text === "string"
        ? item.text
        : typeof item.message === "string"
          ? item.message
          : "";
    const summary =
      type === "tool_call"
        ? String(item.name || "工具")
        : type === "file_change"
          ? String(item.path || "")
          : "";
    return {
      seq,
      type,
      label: labels[type] || "运行事件",
      message: message || (typeof event === "string" ? event : ""),
      summary,
      raw: JSON.stringify(event, null, 2),
    };
  }),
);
</script>
<template>
  <section class="panel task-summary">
    <div class="section-line">
      <span class="agent-label"><AppIcon name="agent" />{{ run.agent }}</span
      ><span class="run-status" :data-state="run.state">{{
        runState(run.state)
      }}</span>
    </div>
    <p class="task-prompt">{{ run.prompt }}</p>
    <div class="output-actions">
      <button
        v-if="run.state === 'running'"
        class="primary-button"
        :disabled="busy || disabled"
        @click="$emit('cancel')"
      >
        <AppIcon name="stop" />停止任务</button
      ><template v-else
        ><button
          v-if="resumable"
          class="primary-button"
          :disabled="busy || disabled"
          @click="$emit('continue')"
        >
          继续会话</button
        ><button
          class="tonal-button"
          :disabled="busy || disabled"
          @click="$emit('remove')"
        >
          删除记录
        </button></template
      >
    </div>
  </section>
  <p v-if="run.state === 'failed'" role="alert" class="error-banner">
    {{ runFailure(run.error_code) }}
  </p>
  <p v-if="run.state === 'interrupted'" class="hint">
    电脑服务重启，任务已中断。可查看已有输出后继续会话。
  </p>
  <h2 class="subheading">运行输出</h2>
  <div v-if="!output.length" class="panel empty-state">
    <AppIcon name="agent" />
    <p>
      {{
        run.state === "running"
          ? "正在等待 Agent 输出…"
          : "任务已结束，没有输出。"
      }}
    </p>
  </div>
  <article
    v-for="item in output"
    :key="item.seq"
    class="event-panel"
    :class="{ 'event-error': item.type === 'error' }"
  >
    <span class="event-label">{{ item.label }}</span>
    <pre v-if="item.message" class="message-text">{{ item.message }}</pre>
    <template v-else
      ><p v-if="item.summary" class="event-summary">{{ item.summary }}</p>
      <details>
        <summary>查看详情</summary>
        <pre>{{ item.raw }}</pre>
      </details></template
    >
  </article>
</template>
