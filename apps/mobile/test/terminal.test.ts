import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { effectScope, type EffectScope } from "vue";
import { RemoteError } from "../src/client";
import { useRemoteTerminal } from "../src/composables/use-remote-terminal";
import {
  terminalSocketUrl,
  terminalTheme,
  type TerminalSession,
  type RemoteRequest,
} from "../src/terminal";
import { THEMES } from "../src/composables/use-appearance";

const first: TerminalSession = {
  id: "one",
  project_id: "project",
  workspace: "/work/project",
  shell: "/bin/sh",
  cols: 80,
  rows: 24,
  state: "running",
  created_at: 100,
  exit_code: null,
};
const second: TerminalSession = { ...first, id: "two" };
let scopes: EffectScope[];
beforeEach(() => {
  vi.useFakeTimers();
  scopes = [];
});
afterEach(() => {
  scopes.forEach((scope) => scope.stop());
  vi.useRealTimers();
});
const flush = async () => {
  for (let index = 0; index < 12; index++) await Promise.resolve();
};

class FakeSocket {
  readyState = 1;
  onopen: ((event: Event) => void) | null = null;
  onmessage: ((event: MessageEvent) => void) | null = null;
  onerror: ((event: Event) => void) | null = null;
  onclose: ((event: CloseEvent) => void) | null = null;
  send = vi.fn();
  close = vi.fn(() => {
    this.readyState = 3;
  });
  receive(packet: unknown) {
    this.onmessage?.({ data: JSON.stringify(packet) } as MessageEvent);
  }
  disconnect() {
    this.readyState = 3;
    this.onclose?.({} as CloseEvent);
  }
}

function fixture(write = vi.fn(async (_data: string) => {})) {
  let current = first;
  const sockets: FakeSocket[] = [];
  const urls: string[] = [];
  const requests = vi.fn(
    async (path: string, method = "GET", _data?: unknown): Promise<unknown> => {
      if (path.endsWith("/ticket")) {
        const id = path.split("/")[2];
        return {
          ticket: "single-use",
          expires_at: 1000,
          ws_path: `/remote/v1/terminals/${id}/ws?ticket=single-use`,
        };
      }
      if (method === "DELETE") return current;
      return current;
    },
  );
  const request: RemoteRequest = <T>(
    path: string,
    method?: string,
    data?: unknown,
  ) => requests(path, method, data) as Promise<T>;
  const scope = effectScope();
  scopes.push(scope);
  const reset = vi.fn();
  const remote = scope.run(() =>
    useRemoteTerminal({
      address: "https://computer.test:7337",
      request,
      write,
      reset,
      openSocket: (url) => {
        urls.push(url);
        const socket = new FakeSocket();
        sockets.push(socket);
        return socket;
      },
    }),
  )!;
  return {
    remote,
    sockets,
    requests,
    write,
    reset,
    urls,
    scope,
    setCurrent: (row: TerminalSession) => {
      current = row;
    },
  };
}

it("uses same-origin ticket URLs and never accepts a credential-bearing redirect", () => {
  expect(
    terminalSocketUrl("https://computer.test", "one", {
      ticket: "once",
      expires_at: 1,
      ws_path: "/remote/v1/terminals/one/ws?ticket=once",
    }),
  ).toBe("wss://computer.test/remote/v1/terminals/one/ws?ticket=once");
  expect(() =>
    terminalSocketUrl("http://computer.test", "one", {
      ticket: "once",
      expires_at: 1,
      ws_path: "https://other.test/remote/v1/terminals/one/ws?ticket=once",
    }),
  ).toThrow();
});

it("keeps the xterm background, text, cursor and ANSI colors consistent with all six themes", () => {
  for (const theme of THEMES) {
    const colors = terminalTheme(theme);
    expect(colors.background).toBe(theme.tokens.background);
    expect(colors.foreground).toBe(theme.tokens.text);
    expect(colors.cursor).toBe(theme.tokens.primary);
    expect(colors.green).toBe(theme.tokens.secondary);
  }
});

it("waits for ready, forwards actual input/control sequences and sends fitted dimensions", async () => {
  const f = fixture();
  f.remote.resize(42, 18);
  f.remote.attach(first);
  await flush();
  expect(f.remote.canInput.value).toBe(false);
  const socket = f.sockets[0]!;
  socket.receive({ type: "ready", terminal: first });
  await flush();
  expect(f.remote.canInput.value).toBe(true);
  f.remote.input("printf hello\r");
  f.remote.input("\u0003");
  f.remote.resize(32, 12);
  expect(socket.send.mock.calls.map((call) => JSON.parse(call[0]))).toEqual([
    { type: "resize", cols: 42, rows: 18 },
    { type: "input", data: "printf hello\r" },
    { type: "input", data: "\u0003" },
    { type: "resize", cols: 32, rows: 12 },
  ]);
  expect(f.urls[0]).not.toContain("Bearer");
});

it("reconnects with the rendered cursor and ignores replayed output sequences", async () => {
  const f = fixture();
  f.remote.attach(first);
  await flush();
  const socket = f.sockets[0]!;
  socket.receive({ type: "ready", terminal: first });
  socket.receive({ type: "output", seq: 1, data: "hello\r\n" });
  await flush();
  socket.disconnect();
  await flush();
  await vi.advanceTimersByTimeAsync(1000);
  await flush();
  expect(f.requests).toHaveBeenLastCalledWith("/terminals/one/ticket", "POST", {
    after: 1,
  });
  const next = f.sockets[1]!;
  next.receive({ type: "ready", terminal: first });
  next.receive({ type: "output", seq: 1, data: "hello\r\n" });
  next.receive({ type: "output", seq: 2, data: "next\r\n" });
  await flush();
  expect(f.write.mock.calls.map((call) => call[0])).toEqual([
    "hello\r\n",
    "next\r\n",
  ]);
});

it("keeps a very short fitted viewport within the PTY protocol minimum dimensions", async () => {
  const f = fixture();
  f.remote.resize(1, 1);
  f.remote.attach(first);
  await flush();
  f.sockets[0]!.receive({ type: "ready", terminal: first });
  await flush();
  expect(JSON.parse(f.sockets[0]!.send.mock.calls[0]![0])).toEqual({
    type: "resize",
    cols: 2,
    rows: 2,
  });
});

it("reports replay gaps and keeps terminal exits read-only without reconnecting", async () => {
  const f = fixture();
  f.remote.attach(first);
  await flush();
  const socket = f.sockets[0]!;
  socket.receive({ type: "ready", terminal: first });
  socket.receive({ type: "gap", first_seq: 5 });
  socket.receive({ type: "output", seq: 5, data: "tail" });
  socket.receive({
    type: "exit",
    terminal: { ...first, state: "exited", exit_code: 0 },
  });
  await flush();
  expect(f.write.mock.calls[0]?.[0]).toContain("历史输出已过期");
  expect(f.remote.connection.value).toBe("finished");
  expect(f.remote.canInput.value).toBe(false);
  f.remote.input("ignored");
  await vi.advanceTimersByTimeAsync(20_000);
  expect(f.sockets).toHaveLength(1);
});

it("waits for accepted output to finish rendering before requesting a reconnect cursor", async () => {
  let complete!: () => void;
  const write = vi.fn(
    (_data: string) =>
      new Promise<void>((resolve) => {
        complete = resolve;
      }),
  );
  const f = fixture(write);
  f.remote.attach(first);
  await flush();
  const socket = f.sockets[0]!;
  socket.receive({ type: "ready", terminal: first });
  socket.receive({ type: "output", seq: 1, data: "accepted output" });
  await flush();
  socket.disconnect();
  await vi.advanceTimersByTimeAsync(5000);
  expect(f.sockets).toHaveLength(1);
  complete();
  await flush();
  await vi.advanceTimersByTimeAsync(1000);
  await flush();
  expect(f.requests).toHaveBeenLastCalledWith("/terminals/one/ticket", "POST", {
    after: 1,
  });
  const next = f.sockets[1]!;
  next.receive({ type: "ready", terminal: first });
  next.receive({ type: "output", seq: 1, data: "accepted output" });
  await flush();
  expect(write).toHaveBeenCalledTimes(1);
});

it("does not retry a stale disconnect after manually reconnecting during a pending write", async () => {
  let complete!: () => void;
  const f = fixture(
    vi.fn(
      (_data: string) =>
        new Promise<void>((resolve) => {
          complete = resolve;
        }),
    ),
  );
  f.remote.attach(first);
  await flush();
  const original = f.sockets[0]!;
  original.receive({ type: "ready", terminal: first });
  original.receive({ type: "output", seq: 1, data: "pending output" });
  await flush();
  original.disconnect();
  f.remote.reconnect();
  await flush();
  const next = f.sockets[1]!;
  next.receive({ type: "ready", terminal: first });
  complete();
  await flush();
  await vi.advanceTimersByTimeAsync(2000);
  expect(f.sockets).toHaveLength(2);
  expect(next.close).not.toHaveBeenCalled();
  expect(f.remote.canInput.value).toBe(true);
});

it.each(["中".repeat(2731), "\u0000".repeat(3000)])(
  "rejects paste exceeding the UTF-8 or JSON frame limit while keeping the session connected",
  async (data) => {
    const f = fixture();
    f.remote.attach(first);
    await flush();
    const socket = f.sockets[0]!;
    socket.receive({ type: "ready", terminal: first });
    await flush();
    socket.send.mockClear();
    f.remote.input(data);
    expect(socket.send).not.toHaveBeenCalled();
    expect(f.remote.error.value).toContain("分段粘贴");
    expect(f.remote.canInput.value).toBe(true);
    f.remote.input("printf ok\r");
    expect(socket.send).toHaveBeenCalledWith(
      JSON.stringify({ type: "input", data: "printf ok\r" }),
    );
    expect(f.remote.error.value).toBe("");
  },
);

it("accepts an input at the 8192 UTF-8 byte boundary", async () => {
  const f = fixture();
  f.remote.attach(first);
  await flush();
  const socket = f.sockets[0]!;
  socket.receive({ type: "ready", terminal: first });
  await flush();
  const data = "中".repeat(2730) + "ok";
  f.remote.input(data);
  expect(socket.send).toHaveBeenLastCalledWith(
    JSON.stringify({ type: "input", data }),
  );
  expect(f.remote.error.value).toBe("");
});

it("drains an accepted xterm write before resetting for another session", async () => {
  let complete!: () => void;
  const write = vi.fn(
    (_data: string) =>
      new Promise<void>((resolve) => {
        complete = resolve;
      }),
  );
  const f = fixture(write);
  f.remote.attach(first);
  await flush();
  f.sockets[0]!.receive({ type: "ready", terminal: first });
  f.sockets[0]!.receive({ type: "output", seq: 1, data: "old screen" });
  await flush();
  f.remote.attach(second);
  await flush();
  expect(f.reset).toHaveBeenCalledTimes(1);
  expect(f.sockets).toHaveLength(1);
  complete();
  await flush();
  expect(f.reset).toHaveBeenCalledTimes(2);
  expect(f.sockets).toHaveLength(2);
  expect(f.remote.terminal.value?.id).toBe("two");
});

it("returning from the page closes only the socket, cancels retry and never deletes the shell", async () => {
  const f = fixture();
  f.remote.attach(first);
  await flush();
  f.sockets[0]!.receive({ type: "ready", terminal: first });
  await flush();
  f.scope.stop();
  await vi.advanceTimersByTimeAsync(30_000);
  expect(f.sockets[0]!.close).toHaveBeenCalledTimes(1);
  expect(f.requests.mock.calls.some((call) => call[1] === "DELETE")).toBe(
    false,
  );
  expect(f.sockets).toHaveLength(1);
});

it.each([401, 404])(
  "does not repeat authentication/expired-session failures: %s",
  async (status) => {
    const f = fixture();
    f.requests.mockRejectedValueOnce(new RemoteError(status, "unavailable"));
    f.remote.attach(first);
    await flush();
    await vi.advanceTimersByTimeAsync(30_000);
    expect(f.remote.connection.value).toBe("disconnected");
    expect(f.requests).toHaveBeenCalledTimes(1);
    expect(f.sockets).toHaveLength(0);
  },
);

it("explicitly closes the process and verifies a pending close with GET", async () => {
  const f = fixture();
  f.remote.attach(first);
  await flush();
  f.sockets[0]!.receive({ type: "ready", terminal: first });
  await flush();
  await f.remote.closeProcess();
  expect(f.remote.closing.value).toBe(true);
  f.setCurrent({ ...first, state: "closed" });
  await vi.advanceTimersByTimeAsync(300);
  await flush();
  expect(f.requests).toHaveBeenCalledWith(
    "/terminals/one",
    "DELETE",
    undefined,
  );
  expect(f.requests).toHaveBeenCalledWith(
    "/terminals/one",
    undefined,
    undefined,
  );
  expect(f.remote.terminal.value?.state).toBe("closed");
  expect(f.remote.closing.value).toBe(false);
});

it("ignores a late ticket from a previously selected session", async () => {
  const f = fixture();
  let complete!: (value: unknown) => void;
  f.requests.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        complete = resolve;
      }),
  );
  f.remote.attach(first);
  await flush();
  f.remote.attach(second);
  await flush();
  complete({
    ticket: "old",
    expires_at: 1,
    ws_path: "/remote/v1/terminals/one/ws?ticket=old",
  });
  await flush();
  expect(f.sockets).toHaveLength(1);
  expect(f.urls[0]).toContain("/two/ws");
});
