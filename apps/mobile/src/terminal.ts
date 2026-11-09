import type { ITheme } from "@xterm/xterm";
import type { AppearanceTheme } from "./composables/use-appearance";

export type TerminalSession = {
  id: string;
  project_id: string;
  workspace: string;
  shell: string;
  cols: number;
  rows: number;
  state: "running" | "exited" | "closed" | "failed";
  created_at: number;
  exit_code: number | null;
};
export type TerminalTicket = {
  ticket: string;
  expires_at: number;
  ws_path: string;
};
export type RemoteRequest = <T>(
  path: string,
  method?: string,
  data?: unknown,
) => Promise<T>;

export function terminalState(session: TerminalSession): string {
  if (session.state === "running") return "运行中";
  if (session.state === "closed") return "已关闭";
  if (session.state === "failed") return "终端启动失败";
  return session.exit_code === null
    ? "已退出"
    : `已退出 · ${session.exit_code}`;
}

export function terminalSocketUrl(
  address: string,
  id: string,
  ticket: TerminalTicket,
): string {
  const base = new URL(address);
  const url = new URL(ticket.ws_path, base);
  if (
    url.origin !== base.origin ||
    url.username ||
    url.password ||
    url.hash ||
    url.pathname !== `/remote/v1/terminals/${encodeURIComponent(id)}/ws` ||
    !url.searchParams.get("ticket")
  ) {
    throw new Error("电脑返回的终端连接地址无效");
  }
  url.protocol = base.protocol === "https:" ? "wss:" : "ws:";
  return url.href;
}

export function terminalTheme(theme: AppearanceTheme): ITheme {
  const t = theme.tokens;
  return {
    background: t.background,
    foreground: t.text,
    cursor: t.primary,
    cursorAccent: t.onPrimary,
    selectionBackground: `${t.primary}44`,
    selectionForeground: t.text,
    black: t.background,
    brightBlack: t.muted,
    red: t.primary,
    brightRed: t.primary,
    green: t.secondary,
    brightGreen: t.secondary,
    yellow: t.tertiary,
    brightYellow: t.tertiary,
    blue: t.secondary,
    brightBlue: t.secondary,
    magenta: t.primary,
    brightMagenta: t.primary,
    cyan: t.secondary,
    brightCyan: t.secondary,
    white: t.text,
    brightWhite: t.text,
  };
}

export const TERMINAL_KEYS = [
  { label: "Tab", data: "\t" },
  { label: "Ctrl+C", data: "\u0003" },
  { label: "Esc", data: "\u001b" },
  { label: "↑", data: "\u001b[A", aria: "上方向键" },
  { label: "↓", data: "\u001b[B", aria: "下方向键" },
  { label: "←", data: "\u001b[D", aria: "左方向键" },
  { label: "→", data: "\u001b[C", aria: "右方向键" },
  { label: "Enter", data: "\r" },
] as const;
