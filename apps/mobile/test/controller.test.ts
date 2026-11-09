import { effectScope } from "vue";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { useRemoteControl } from "../src/composables/use-remote-control";
import { RemoteClient, RemoteError } from "../src/client";
import { credentials } from "../src/credentials";
let scope: ReturnType<typeof effectScope>;
let control: ReturnType<typeof useRemoteControl>;
let request: ReturnType<typeof vi.spyOn>;
const run = {
  id: "one",
  agent: "codex",
  prompt: "task",
  state: "completed",
  session_id: "session",
};
beforeEach(async () => {
  vi.useFakeTimers();
  vi.stubGlobal("localStorage", { getItem: () => null, setItem: vi.fn() });
  vi.spyOn(credentials, "get").mockResolvedValue({ value: "token" });
  request = vi
    .spyOn(RemoteClient.prototype, "request")
    .mockImplementation(async (path) => {
      if (path === "/info") return { name: "computer" };
      if (path === "/agents")
        return {
          agents: [{ name: "codex", available: true, supports_resume: true }],
        };
      return { runs: [run] };
    });
  scope = effectScope();
  control = scope.run(() => useRemoteControl())!;
  control.address.value = "localhost";
  await control.connect();
});
afterEach(() => {
  scope.stop();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});
it("drains terminal pages to an empty page before stopping", async () => {
  request
    .mockResolvedValueOnce({
      events: [{ seq: 1, event: "first" }],
      next_cursor: 1,
      state: "completed",
    })
    .mockResolvedValueOnce({
      events: [{ seq: 2, event: "last" }],
      next_cursor: 2,
      state: "completed",
    })
    .mockResolvedValueOnce({ events: [], next_cursor: 2, state: "completed" })
    .mockResolvedValueOnce({ runs: [run] });
  await control.open(run);
  await vi.runAllTimersAsync();
  expect(control.events.value.map((e) => e.event)).toEqual(["first", "last"]);
  expect(request).toHaveBeenCalledWith("/runs/one/events?after=2");
  expect(vi.getTimerCount()).toBe(0);
});
it("marks the device as needing pairing when a terminal request rejects authentication", async () => {
  request.mockRejectedValueOnce(new RemoteError(401, "expired"));
  await expect(control.remoteRequest("/terminals")).rejects.toMatchObject({
    status: 401,
  });
  expect(control.connectionStatus.value).toBe("needs_pairing");
});
it("retries an interrupted read without skipping its cursor", async () => {
  request
    .mockRejectedValueOnce(new Error("offline"))
    .mockResolvedValueOnce({ events: [], next_cursor: 0, state: "completed" })
    .mockResolvedValueOnce({ runs: [run] });
  await control.open(run);
  expect(control.error.value).toContain("正在重试");
  await vi.advanceTimersByTimeAsync(3000);
  expect(
    request.mock.calls.filter(
      (c: unknown[]) => c[0] === "/runs/one/events?after=0",
    ),
  ).toHaveLength(2);
  expect(control.error.value).toBe("");
});
it("discards an old task response after switching tasks", async () => {
  let resolve!: (value: unknown) => void;
  request
    .mockImplementationOnce(() => new Promise((r) => (resolve = r)))
    .mockResolvedValueOnce({ events: [], next_cursor: 0, state: "completed" })
    .mockResolvedValueOnce({ runs: [{ ...run, id: "two" }] });
  const first = control.open(run);
  await control.open({ ...run, id: "two" });
  resolve({
    events: [{ seq: 1, event: "stale" }],
    next_cursor: 1,
    state: "completed",
  });
  await first;
  expect(control.selected.value?.id).toBe("two");
  expect(control.events.value).toEqual([]);
});
it("reuses the submission key after a lost response and forwards continuation", async () => {
  control.selected.value = run;
  control.continueRun();
  control.prompt.value = "continue";
  request.mockRejectedValueOnce(new Error("lost response"));
  await control.submit();
  request.mockRejectedValueOnce(new Error("still offline"));
  await control.submit();
  const posts = request.mock.calls.filter((c: unknown[]) => c[1] === "POST");
  expect(posts).toHaveLength(2);
  expect(posts[0]?.[2]).toEqual(posts[1]?.[2]);
  expect(posts[0]?.[2]).toMatchObject({ resume: "session" });
});
it("stops retrying expired authentication and permits explicit re-pair", async () => {
  request.mockRejectedValueOnce(new RemoteError(401, "expired"));
  await control.open(run);
  expect(vi.getTimerCount()).toBe(0);
  vi.spyOn(RemoteClient.prototype, "pair").mockResolvedValue({
    token: "new",
    device_id: "phone",
    expires_at: 100,
  });
  vi.spyOn(credentials, "set").mockResolvedValue();
  control.code.value = "new-code";
  await control.connect();
  expect(control.connected.value).toBe("http://localhost:7337");
  expect(control.error.value).toBe("");
});
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((r) => (resolve = r));
  return { promise, resolve };
}
it("keeps a new device pairing code when an old credential write finishes", async () => {
  const save = deferred<void>(),
    read = deferred<{ value: string | null }>();
  vi.spyOn(RemoteClient.prototype, "pair").mockResolvedValue({
    token: "new",
    device_id: "phone",
    expires_at: 100,
  });
  vi.spyOn(credentials, "set").mockReturnValueOnce(save.promise);
  control.code.value = "first";
  const first = control.connect();
  await vi.waitFor(() => expect(credentials.set).toHaveBeenCalled());
  vi.mocked(credentials.get).mockReturnValueOnce(read.promise);
  control.address.value = "second";
  control.code.value = "second-code";
  const second = control.connect();
  save.resolve();
  await first;
  expect(control.code.value).toBe("second-code");
  read.resolve({ value: "saved" });
  await second;
  expect(control.connected.value).toBe("http://second:7337");
});
it("keeps the new device retry key when an old submission succeeds", async () => {
  const old = deferred<unknown>();
  request.mockReturnValueOnce(old.promise);
  control.prompt.value = "old task";
  const first = control.submit();
  control.address.value = "second";
  await control.connect();
  control.prompt.value = "new task";
  request.mockRejectedValueOnce(new Error("lost response"));
  await control.submit();
  old.resolve(run);
  await first;
  request.mockRejectedValueOnce(new Error("offline"));
  await control.submit();
  const posts = request.mock.calls.filter((c: unknown[]) => c[1] === "POST");
  expect(posts[1]?.[2]).toEqual(posts[2]?.[2]);
});
it("drops output received from the previous computer", async () => {
  const old = deferred<unknown>();
  request.mockReturnValueOnce(old.promise);
  const opening = control.open(run);
  control.address.value = "second";
  await control.connect();
  old.resolve({
    events: [{ seq: 1, event: "old computer" }],
    next_cursor: 1,
    state: "running",
  });
  await opening;
  expect(control.events.value).toEqual([]);
  expect(control.selected.value).toBeNull();
  expect(vi.getTimerCount()).toBe(0);
});
it("cleans pending polling when the scope is disposed", async () => {
  request.mockResolvedValueOnce({
    events: [],
    next_cursor: 0,
    state: "running",
  });
  await control.open({ ...run, state: "running" });
  expect(vi.getTimerCount()).toBe(1);
  const calls = request.mock.calls.length;
  scope.stop();
  await vi.runAllTimersAsync();
  expect(request.mock.calls).toHaveLength(calls);
  expect(vi.getTimerCount()).toBe(0);
});

it("targets the selected saved computer when its browser credential is missing", async () => {
  vi.mocked(credentials.get).mockResolvedValueOnce({ value: null });
  control.code.value = "code-for-previous-computer";
  await control.connect({ address: "http://second:7337", name: "second computer" });
  expect(control.address.value).toBe("http://second:7337");
  expect(control.code.value).toBe("");
  expect(control.connected.value).toBe("");
  expect(control.error.value).toContain("配对码");
});

it("preserves submission errors while background output continues arriving", async () => {
  request.mockResolvedValueOnce({ events: [], next_cursor: 0, state: "running" });
  await control.open({ ...run, state: "running" });
  control.prompt.value = "another task";
  request.mockRejectedValueOnce(new RemoteError(429, "任务数量达到上限"));
  await control.submit();
  request.mockResolvedValueOnce({
    events: [{ seq: 1, event: "ongoing output" }],
    next_cursor: 1,
    state: "running",
  });
  await vi.advanceTimersByTimeAsync(1000);
  expect(control.error.value).toBe("任务数量达到上限");
});

it("updates the visible connection state when a device token is revoked", async () => {
  request.mockRejectedValueOnce(new RemoteError(401, "配对已过期"));
  await control.open(run);
  expect(control.connectionStatus.value).toBe("needs_pairing");
  expect(control.error.value).toBe("配对已过期");
  expect(vi.getTimerCount()).toBe(0);
});

it("keeps a draft edited while the previous submission is in flight", async () => {
  const started = deferred<unknown>();
  request.mockReturnValueOnce(started.promise);
  control.prompt.value = "first task";
  const submitting = control.submit();
  control.prompt.value = "next draft";
  request.mockResolvedValueOnce({ events: [], next_cursor: 0, state: "completed" });
  request.mockResolvedValueOnce({ runs: [run] });
  started.resolve(run);
  await submitting;
  expect(control.prompt.value).toBe("next draft");
});

it("closes a selected task removed by another device when refreshing history", async () => {
  request.mockResolvedValueOnce({ events: [], next_cursor: 0, state: "running" });
  await control.open({ ...run, state: "running" });
  expect(vi.getTimerCount()).toBe(1);
  request.mockResolvedValueOnce({ runs: [] });
  await control.refresh();
  expect(control.selected.value).toBeNull();
  expect(control.events.value).toEqual([]);
  expect(control.runs.value).toEqual([]);
  expect(vi.getTimerCount()).toBe(0);
});

it("keeps newer history when an older refresh finishes later", async () => {
  const older = deferred<unknown>();
  control.selected.value = { ...run, state: "running" };
  request.mockReturnValueOnce(older.promise);
  const first = control.refresh();
  request.mockResolvedValueOnce({ runs: [{ ...run, state: "completed" }] });
  await control.refresh();
  older.resolve({ runs: [{ ...run, state: "running" }] });
  await first;
  expect(control.runs.value[0]?.state).toBe("completed");
  expect(control.selected.value?.state).toBe("completed");
});

it("does not undo terminal output with a history snapshot taken before completion", async () => {
  const older = deferred<unknown>();
  request.mockReturnValueOnce(older.promise);
  const refreshing = control.refresh();
  request.mockResolvedValueOnce({
    events: [{ seq: 1, event: "finished output" }],
    next_cursor: 1,
    state: "completed",
  });
  await control.open({ ...run, state: "running" });
  older.resolve({ runs: [{ ...run, state: "running", session_id: null }] });
  await refreshing;
  expect(control.selected.value?.state).toBe("completed");
  expect(control.selected.value?.session_id).toBe("session");
  expect(control.runs.value[0]?.state).toBe("completed");
});

it("ignores an older refresh failure after a newer refresh succeeds", async () => {
  let fail!: (error: Error) => void;
  request.mockReturnValueOnce(new Promise((_, reject) => { fail = reject; }));
  const first = control.refresh();
  request.mockResolvedValueOnce({ runs: [{ ...run, state: "completed" }] });
  await control.refresh();
  fail(new Error("old disconnected request"));
  await expect(first).resolves.toBeUndefined();
  expect(control.connectionStatus.value).toBe("connected");
});

it("shows an accepted task even if its history refresh fails", async () => {
  const accepted = { ...run, id: "accepted", prompt: "new task", state: "running" };
  control.prompt.value = "new task";
  request.mockImplementation(async (path: string, method?: string) => {
    if (path === "/runs" && method === "POST") return accepted;
    if (path === "/runs") throw new Error("history unavailable");
    return { events: [], next_cursor: 0, state: "running" };
  });
  await control.submit();
  expect(control.selected.value?.id).toBe("accepted");
  expect(control.runs.value.some((task) => task.id === "accepted")).toBe(true);
  expect(control.prompt.value).toBe("");
  expect(request.mock.calls.filter((call: unknown[]) => call[1] === "POST")).toHaveLength(1);
});

it("removes a selected task that another device deleted during output polling", async () => {
  control.runs.value = [run];
  request.mockRejectedValueOnce(new RemoteError(404, "任务已被其他设备删除"));
  await control.open(run);
  expect(control.selected.value).toBeNull();
  expect(control.runs.value).toEqual([]);
  expect(vi.getTimerCount()).toBe(0);
  expect(control.error.value).toContain("其他设备删除");
  expect(control.connectionStatus.value).toBe("connected");
});
