<script setup lang="ts">
import { computed } from "vue";
import { AgentToolIcon, agentIdentity } from "@orchester/design";
import {
  agentHasManagedActivity,
  agentIsMonitorOnly,
  agentPresenceCountKey,
  agentPresentedActivity,
  agentProcessSourcesComplete,
  agentWindowCount,
  type AgentFleetSnapshotDto,
  type AgentRuntimeSummaryDto,
} from "@orchester/protokoll";

const props = defineProps<{
  snapshot: AgentFleetSnapshotDto | null;
  status: "idle" | "loading" | "ready" | "stale" | "error";
  error: string;
}>();
const rows = computed(() => (props.snapshot?.agents ?? []).map((agent) => props.status === "ready" ? agent : {
  ...agent,
  activity: "unknown" as const,
  active_windows: 0,
  active_sessions: 0,
  active_runs: 0,
  active_subagents: 0,
  process_sources: agent.process_sources?.map((source) => source.status === "available" ? { ...source, status: "stale" as const } : source) ?? null,
}));
function count(agent: AgentRuntimeSummaryDto) {
  const value = agentWindowCount(agent);
  return value.count === null ? "—" : value.quality === "lower_bound" ? `至少 ${value.count}` : String(value.count);
}
function unit(agent: AgentRuntimeSummaryDto) {
  return { windows: "窗口", processes: "进程", services: "服务进程" }[agentPresenceCountKey(agent)];
}
function activity(agent: AgentRuntimeSummaryDto) {
  const state = agentPresentedActivity(agent);
  const processOnly = !agentHasManagedActivity(agent) && (agent.capabilities.includes("process_presence") || agent.window_count_source === "external_processes");
  if (processOnly && state === "running") return "服务运行";
  if (processOnly && (state === "idle" || state === "offline")) return "未发现运行进程";
  return {
    running: "运行中", idle: "空闲", offline: "离线", unknown: "状态未知",
    starting: "正在启动", stopping: "正在停止", waiting_approval: "等待批准", error: "状态异常",
  }[state];
}
</script>

<template>
  <section class="agent-monitor-list" data-mobile-agent-monitors aria-label="电脑 Agent 状态">
    <h3>Agent 状态</h3>
    <p v-if="status === 'loading'" class="hint" role="status">正在读取 Agent 状态…</p>
    <p v-else-if="status === 'idle'" class="hint" role="status">连接电脑后可查看 Agent 状态。</p>
    <p v-if="error" class="hint error-message" role="status">{{ error }}</p>
    <p v-if="status === 'stale' && snapshot" class="hint">保留上次列表，当前运行状态尚未确认。</p>
    <ul v-if="rows.length">
      <li v-for="agent in rows" :key="agent.agent_id" :data-mobile-agent-id="agent.agent_id">
        <AgentToolIcon :icon-key="agentIdentity(agent.provider)?.key ?? agent.icon_key" />
        <span class="agent-monitor-copy">
          <strong>{{ agent.display_name }}</strong>
          <small data-mobile-agent-activity>{{ activity(agent) }}</small>
        </span>
        <span class="agent-monitor-count" :data-mobile-agent-count-unit="agentPresenceCountKey(agent)">
          <strong data-mobile-agent-count>{{ count(agent) }}</strong>{{ unit(agent) }}
        </span>
        <small v-if="agentIsMonitorOnly(agent)" class="agent-monitor-note">仅监控；服务存在不代表任务执行。</small>
        <details v-if="!agentProcessSourcesComplete(agent)" class="agent-monitor-sources" data-mobile-agent-process-sources>
          <summary>{{ agent.process_sources === null ? '进程来源尚未确认。' : '进程监控不完整。' }}</summary>
          <ul v-if="agent.process_sources !== null">
            <li v-for="(source, index) in agent.process_sources" :key="index" :data-mobile-process-source-status="source.status">
              {{ { local: '本机', windows: 'Windows', wsl: 'WSL' }[source.environment] }}：
              {{ source.status === 'unavailable' ? '来源不可用' : source.status === 'stale' ? `上次观测：${source.count}` : `当前：${source.count}` }}
            </li>
          </ul>
        </details>
      </li>
    </ul>
    <p v-else-if="status === 'ready'" class="hint">这台电脑没有可显示的 Agent。</p>
  </section>
</template>

<style scoped>
.agent-monitor-list { margin-block-start: 16px; padding-block-start: 12px; border-block-start: 1px solid var(--color-border-base); }
.agent-monitor-list h3 { margin: 0 0 12px; font-size: 14px; }
.agent-monitor-list ul { display: grid; gap: 12px; margin: 0; padding: 0; list-style: none; }
.agent-monitor-list li { display: grid; grid-template-columns: 28px minmax(0, 1fr) auto; align-items: center; column-gap: 10px; }
.agent-monitor-copy { display: grid; min-inline-size: 0; gap: 3px; }
.agent-monitor-copy strong { overflow-wrap: anywhere; font-size: 14px; }
.agent-monitor-copy small, .agent-monitor-count, .agent-monitor-note { color: var(--color-text-secondary); font-size: 12px; }
.agent-monitor-count { display: grid; justify-items: end; gap: 3px; }
.agent-monitor-count strong { color: var(--color-text-primary); }
.agent-monitor-note { grid-column: 2 / -1; margin-block-start: 3px; }
.agent-monitor-sources { grid-column: 2 / -1; margin-block-start: 3px; color: var(--color-text-secondary); font-size: 12px; }
.agent-monitor-sources summary { cursor: pointer; }
.agent-monitor-sources ul { gap: 3px; margin-block-start: 4px; }
.agent-monitor-sources li { display: block; }
</style>
