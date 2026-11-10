import { computed, onScopeDispose, ref, shallowRef } from "vue";
import { RemoteError } from "../client";
import {
  terminalSocketUrl,
  type RemoteRequest,
  type TerminalSession,
  type TerminalTicket,
} from "../terminal";

type Socket = Pick<
  WebSocket,
  | "readyState"
  | "send"
  | "close"
  | "onopen"
  | "onmessage"
  | "onerror"
  | "onclose"
>;
type TerminalOptions = {
  address: string;
  request: RemoteRequest;
  write: (data: string) => Promise<void>;
  reset: () => void;
  openSocket?: (url: string) => Socket;
};
const encoder = new TextEncoder();

export function useRemoteTerminal(options: TerminalOptions) {
  const terminal = shallowRef<TerminalSession | null>(null);
  const connection = ref<
    | "idle"
    | "connecting"
    | "connected"
    | "reconnecting"
    | "disconnected"
    | "finished"
  >("idle");
  const error = ref("");
  const closing = ref(false);
  const canInput = computed(
    () =>
      connection.value === "connected" &&
      terminal.value?.state === "running" &&
      !closing.value,
  );
  let socket: Socket | null = null;
  let generation = 0;
  let transport = 0;
  let sequence = 0;
  let attempt = 0;
  let disposed = false;
  let receivedExit = false;
  let queue = Promise.resolve();
  let retryTimer: ReturnType<typeof setTimeout> | undefined;
  let openTimer: ReturnType<typeof setTimeout> | undefined;
  let closeTimer: ReturnType<typeof setTimeout> | undefined;
  let size = { cols: 80, rows: 24 };

  function releaseSocket() {
    transport++;
    clearTimeout(retryTimer);
    clearTimeout(openTimer);
    retryTimer = openTimer = undefined;
    const previous = socket;
    socket = null;
    if (previous) {
      previous.onopen =
        previous.onmessage =
        previous.onerror =
        previous.onclose =
          null;
      previous.close();
    }
  }

  function retry() {
    if (disposed || !terminal.value || receivedExit) return;
    connection.value = "reconnecting";
    clearTimeout(retryTimer);
    const delay = Math.min(15_000, 1000 * 2 ** Math.min(attempt++, 4));
    retryTimer = setTimeout(() => {
      void connect();
    }, delay);
  }

  function report(reason: unknown) {
    if (reason instanceof RemoteError && reason.status === 404) {
      error.value = "终端已过期或被关闭，请新建一个终端";
      connection.value = "disconnected";
      return false;
    }
    if (reason instanceof RemoteError && reason.status === 401) {
      error.value = "配对已过期，请重新配对这台电脑";
      connection.value = "disconnected";
      return false;
    }
    error.value = reason instanceof Error ? reason.message : "终端连接中断";
    return true;
  }

  async function connect() {
    releaseSocket();
    const current = terminal.value;
    if (!current || disposed) return;
    const epoch = transport;
    const session = generation;
    connection.value = attempt ? "reconnecting" : "connecting";
    try {
      const ticket = await options.request<TerminalTicket>(
        `/terminals/${encodeURIComponent(current.id)}/ticket`,
        "POST",
        { after: sequence },
      );
      if (disposed || epoch !== transport || session !== generation) return;
      const next = (options.openSocket ?? ((url) => new WebSocket(url)))(
        terminalSocketUrl(options.address, current.id, ticket),
      );
      socket = next;
      openTimer = setTimeout(() => {
        if (socket !== next) return;
        releaseSocket();
        error.value = "终端连接超时，正在重连";
        retry();
      }, 10_000);
      next.onopen = () => {};
      next.onmessage = (event) => {
        if (socket !== next) return;
        queue = queue
          .then(async () => {
            if (disposed || session !== generation) return;
            const message: unknown = JSON.parse(String(event.data));
            if (!message || typeof message !== "object")
              throw new Error("无法识别终端响应");
            const packet = message as Record<string, unknown>;
            if (
              packet.type !== "output" &&
              packet.type !== "gap" &&
              epoch !== transport
            )
              return;
            if (packet.type === "ready") {
              const row = packet.terminal as TerminalSession;
              if (!row || row.id !== current.id)
                throw new Error("终端会话不匹配");
              terminal.value = row;
              clearTimeout(openTimer);
              connection.value = "connected";
              error.value = "";
              attempt = 0;
              if (row.state === "running") send({ type: "resize", ...size });
            } else if (packet.type === "output") {
              if (
                !Number.isSafeInteger(packet.seq) ||
                typeof packet.data !== "string"
              )
                throw new Error("无法识别终端输出");
              const seq = packet.seq as number;
              if (seq <= sequence) return;
              await options.write(packet.data);
              if (!disposed && session === generation) sequence = seq;
            } else if (packet.type === "gap") {
              if (
                !Number.isSafeInteger(packet.first_seq) ||
                (packet.first_seq as number) < 1
              )
                throw new Error("无法识别终端回放位置");
              await options.write(
                "\r\n[部分历史输出已过期，显示保留的输出]\r\n",
              );
              if (!disposed && session === generation)
                sequence = Math.max(sequence, (packet.first_seq as number) - 1);
            } else if (packet.type === "exit") {
              const row = packet.terminal as TerminalSession;
              if (!row || row.id !== current.id)
                throw new Error("终端会话不匹配");
              terminal.value = row;
              receivedExit = true;
              connection.value = "finished";
              error.value = "";
              closing.value = false;
              clearTimeout(closeTimer);
              releaseSocket();
            } else if (packet.type === "error") {
              error.value =
                packet.code === "input_busy"
                  ? "电脑暂时无法接收输入，请稍后重试"
                  : packet.code === "invalid_message"
                    ? "终端输入未被电脑接受"
                    : packet.code === "terminal_closed"
                      ? "电脑终端已结束，正在读取最后输出"
                      : "电脑终端读取失败";
            }
          })
          .catch((reason) => {
            if (disposed || session !== generation) return;
            error.value =
              reason instanceof Error ? reason.message : "无法识别终端响应";
            releaseSocket();
            retry();
          });
      };
      next.onerror = () => {
        if (socket === next) error.value = "终端连接中断，正在重连";
      };
      next.onclose = () => {
        if (socket !== next) return;
        socket = null;
        clearTimeout(openTimer);
        if (!receivedExit) connection.value = "reconnecting";
        // Accepted output is still queued for xterm. Replayed frames are deduplicated
        // after that queue drains, even if the ticket was requested before a write completed.
        queue = queue.then(() => {
          if (
            !disposed &&
            session === generation &&
            epoch === transport &&
            !receivedExit
          )
            retry();
        });
      };
    } catch (reason) {
      if (disposed || epoch !== transport || session !== generation) return;
      if (report(reason)) retry();
    }
  }

  function attach(row: TerminalSession) {
    const previousWrites = queue;
    releaseSocket();
    clearTimeout(closeTimer);
    generation++;
    sequence = 0;
    attempt = 0;
    receivedExit = false;
    closing.value = false;
    terminal.value = row;
    connection.value = "connecting";
    error.value = "";
    const session = generation;
    // Drain accepted xterm writes before resetting the screen for another session.
    queue = previousWrites.then(() => {
      if (disposed || session !== generation) return;
      options.reset();
      void connect();
    });
  }

  function send(message: unknown): boolean {
    if (!socket || socket.readyState !== 1) return false;
    try {
      socket.send(JSON.stringify(message));
      return true;
    } catch {
      error.value = "终端连接中断，正在重连";
      releaseSocket();
      retry();
      return false;
    }
  }

  function input(data: string) {
    if (!canInput.value || !data) return;
    if (
      encoder.encode(data).byteLength > 8192 ||
      encoder.encode(JSON.stringify({ type: "input", data })).byteLength >
        16 * 1024
    ) {
      error.value = "一次输入过长，请分段粘贴（每次最多 8 KiB）";
      return;
    }
    if (send({ type: "input", data })) error.value = "";
  }

  function resize(cols: number, rows: number) {
    size = {
      cols: Math.max(2, Math.min(400, cols)),
      rows: Math.max(2, Math.min(200, rows)),
    };
    if (connection.value === "connected" && terminal.value?.state === "running")
      send({ type: "resize", ...size });
  }

  async function closeProcess() {
    const current = terminal.value;
    if (!current || closing.value || current.state !== "running") return;
    const session = generation;
    closing.value = true;
    error.value = "";
    try {
      const started = Date.now();
      const row = await options.request<TerminalSession>(
        `/terminals/${encodeURIComponent(current.id)}`,
        "DELETE",
      );
      if (disposed || session !== generation || receivedExit) return;
      terminal.value = row;
      const poll = async () => {
        if (disposed || session !== generation || !closing.value) return;
        if (Date.now() - started > 10_000) {
          closing.value = false;
          error.value = "电脑仍在关闭终端，稍后重连查看状态";
          return;
        }
        const updated = await options.request<TerminalSession>(
          `/terminals/${encodeURIComponent(current.id)}`,
        );
        if (disposed || session !== generation || receivedExit) return;
        terminal.value = updated;
        if (updated.state !== "running") {
          // HTTP confirms the process stopped, but only the websocket exit
          // confirms its final output drained. Keep receiving or replaying it.
          closing.value = false;
          error.value = "";
        } else
          closeTimer = setTimeout(() => {
            void poll().catch(failed);
          }, 500);
      };
      const failed = (reason: unknown) => {
        if (!disposed && session === generation && !receivedExit) {
          closing.value = false;
          report(reason);
        }
      };
      if (row.state === "running")
        closeTimer = setTimeout(() => {
          void poll().catch(failed);
        }, 300);
      else {
        closing.value = false;
        error.value = "";
      }
    } catch (reason) {
      if (!disposed && session === generation && !receivedExit) {
        closing.value = false;
        report(reason);
      }
    }
  }

  function reconnect() {
    if (!terminal.value || disposed) return;
    attempt = 0;
    receivedExit = false;
    void connect();
  }

  onScopeDispose(() => {
    disposed = true;
    generation++;
    releaseSocket();
    clearTimeout(closeTimer);
  });
  return {
    terminal,
    connection,
    error,
    closing,
    canInput,
    attach,
    input,
    resize,
    reconnect,
    closeProcess,
  };
}
