const states: Record<string, string> = {
  running: "运行中",
  completed: "已完成",
  failed: "失败",
  cancelled: "已停止",
  interrupted: "已中断",
};

const failures: Record<string, string> = {
  agent_start_failed: "Agent 未能启动。请在电脑检查 CLI 是否可执行、工作区是否存在，以及登录和模型配置是否完成。",
  agent_stream_failed: "Agent 输出连接中断。请在电脑检查 CLI 的登录、模型配置和错误日志，再重试任务。",
  agent_reported_error: "Agent 返回错误。请检查下方输出；需要登录或交互授权时，请在电脑完成。",
  event_limit_exceeded: "任务输出条数已达到上限，执行已停止。请缩小任务范围后重试。",
  output_limit_exceeded: "任务输出大小已达到上限，执行已停止。请缩小任务范围后重试。",
  invalid_event: "Agent 返回了无法处理的输出。请在电脑检查 Agent 版本和日志。",
  storage_failed: "电脑无法保存任务输出。请检查电脑磁盘空间和目录权限。",
};

export function runState(state: string): string {
  return states[state] ?? state;
}

export function runFailure(code?: string | null): string {
  return (code && failures[code]) || "任务执行失败。请检查下方输出和电脑端 Agent 日志。";
}
