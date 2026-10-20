// dev-bridge 契约测试（离线）：原语输出/错误文本与旧 MCP 对齐，能力描述符不变。
// 用「假 api」注入，不发 HTTP；覆盖 get_progress 的计划文档解析与审批双语义。
import assert from "node:assert/strict";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { createPrimitives, CAPABILITIES } from "../dev-bridge/src/primitives.ts";
import { countPlanItems } from "../dev-bridge/src/plan.ts";
import { dispatch, TOOLS } from "../dev-bridge/src/main.ts";

type Ok<T> = { ok: true; status: number; data: T; error: null };
type Err = { ok: false; status: number; data: null; error: string };
const ok = <T>(data: T): Ok<T> => ({ ok: true, status: 200, data, error: null });
const err = (msg: string, status = 404): Err => ({ ok: false, status, data: null, error: msg });

const task = (over: Record<string, unknown> = {}) => ({
  session_id: "s1",
  group_id: "g1",
  qq_id: "dev1",
  title: "做点事",
  plan_path: null as string | null,
  status: "awaiting_confirm",
  created_at: "1",
  updated_at: "1",
  ...over,
});

function fakeApi(over: Partial<Record<string, any>> = {}) {
  const calls: Array<{ fn: string; args: unknown[] }> = [];
  const base = {
    listTasks: async (status?: string) => {
      calls.push({ fn: "listTasks", args: [status] });
      return ok([task()]);
    },
    getTask: async (id: string) => {
      calls.push({ fn: "getTask", args: [id] });
      return ok(task());
    },
    createTask: async (s: string, g: string, q: string) => {
      calls.push({ fn: "createTask", args: [s, g, q] });
      return ok(task({ status: "queued" }));
    },
    transition: async (s: string, to: string, qq?: string) => {
      calls.push({ fn: "transition", args: [s, to, qq] });
      return ok(task({ status: to }));
    },
    confirm: async (s: string, q: string) => {
      calls.push({ fn: "confirm", args: [s, q] });
      return ok(task({ status: "running" }));
    },
    updateMeta: async (s: string, m: unknown) => {
      calls.push({ fn: "updateMeta", args: [s, m] });
      return ok(task());
    },
    permission: async (op: string, q: string, s?: string) => {
      calls.push({ fn: "permission", args: [op, q, s] });
      return ok({ allow: true, reason: null });
    },
    startRun: async (s: string, stage: string) => {
      calls.push({ fn: "startRun", args: [s, stage] });
      return ok({ run_id: `${s}:${stage}:r1`, status: "queued" });
    },
    setRunStatus: async (runId: string, status: string, ref?: string, err?: string) => {
      calls.push({ fn: "setRunStatus", args: [runId, status, ref, err] });
      return ok({ run_id: runId, status });
    },
    activeRun: async (s: string) => {
      calls.push({ fn: "activeRun", args: [s] });
      return ok({ run_id: `${s}:execute:r1`, status: "running", stage: "execute" });
    },
    listEvents: async (s: string, since: number, limit: number) => {
      calls.push({ fn: "listEvents", args: [s, since, limit] });
      return ok([{ id: 1, event_id: "e1", kind: "created" }]);
    },
    ...over,
  };
  return { api: base as any, calls };
}

// ---- 能力描述符：11 项、schema 字段齐全、名称不漂移 ----
assert.equal(CAPABILITIES.length, 11, "能力项数应为 11");
for (const c of CAPABILITIES) {
  for (const k of ["name", "rw", "description", "usage", "example"]) {
    assert.ok((c as any)[k], `${c.name} 缺字段 ${k}`);
  }
  assert.ok(["读", "写"].includes(c.rw), `${c.name} rw 非法`);
}
assert.deepEqual(
  CAPABILITIES.map((c) => c.name),
  [
    "list_tasks",
    "get_task",
    "get_progress",
    "prepare_task_link",
    "append_instruction",
    "list_capabilities",
    "create_task",
    "confirm_task",
    "cancel_task",
    "transition_task",
    "update_task_meta",
  ],
  "能力名称/顺序是对外契约，不得随意改动",
);
assert.equal(TOOLS.length, 18, "MCP 工具数 = 11 旧 + 7 v2（approve_plan/approve_verify/list_events/check_permission/start_run/finish_run/cancel_run）");

// ---- 计划解析：分区隔离 + [x]/[X]/[ ] ----
const planText = [
  "## 计划",
  "- 不该被统计",
  "## 待办实现",
  "- [x] a",
  "- [X] b",
  "- [ ] c",
  "## 验证事项",
  "- [x] v1",
  "- [ ] v2",
  "## 其它",
  "- [x] x",
].join("\n");
assert.deepEqual(countPlanItems(planText, "待办实现"), { done: 2, total: 3 });
assert.deepEqual(countPlanItems(planText, "验证事项"), { done: 1, total: 2 });

// ---- get_progress：读不到计划 → 0/0；读到 → 计数 ----
const dir = mkdtempSync(join(tmpdir(), "dev-bridge-"));
try {
  const planPath = join(dir, "p.md");
  writeFileSync(planPath, planText);
  const { api } = fakeApi({
    getTask: async () => ok(task({ plan_path: planPath })),
  });
  const out = JSON.parse(await createPrimitives(api).get_progress({ session_id: "s1" }));
  assert.equal(out.status, "awaiting_confirm");
  assert.deepEqual(out.todo, { done: 2, total: 3 });
  assert.deepEqual(out.verify, { done: 1, total: 2 });

  const missing = fakeApi({ getTask: async () => ok(task({ plan_path: join(dir, "nope.md") })) });
  const out2 = JSON.parse(await createPrimitives(missing.api).get_progress({ session_id: "s1" }));
  assert.deepEqual(out2.todo, { done: 0, total: 0 });
} finally {
  rmSync(dir, { recursive: true, force: true });
}

// ---- 错误文本：统一 `错误: ` 前缀，不透出裸 HTTP 语义 ----
{
  const { api } = fakeApi({ getTask: async () => err("任务不存在") });
  const out = await createPrimitives(api).get_task({ session_id: "nope" });
  assert.equal(out, "错误: 任务不存在");
}

// ---- 缺参：抛错由 dispatch 兜底成 `错误: ...` ----
{
  const { api } = fakeApi();
  const prims = createPrimitives(api);
  const out = await dispatch(prims, "get_task", {});
  assert.match(out, /^错误: 缺少参数 session_id/);
  const unknown = await dispatch(prims, "no_such_tool", {});
  assert.match(unknown, /^错误: 未知原语/);
}

// ---- 审批双语义：stage 不符即拒，且不调用 confirm ----
{
  const { api, calls } = fakeApi({ getTask: async () => ok(task({ status: "verifying" })) });
  const prims = createPrimitives(api);
  const bad = await prims.approve_plan({ session_id: "s1", qq_id: "dev1" });
  assert.match(bad, /^错误: 非法状态迁移: verifying/);
  assert.equal(calls.filter((c) => c.fn === "confirm").length, 0);

  const good = await prims.approve_verify({ session_id: "s1", qq_id: "dev1" });
  assert.ok(good.startsWith("{"), "批准应返回任务 JSON");
  assert.deepEqual(calls.find((c) => c.fn === "confirm")?.args, ["s1", "dev1"]);
}

// ---- 归属/权限：桥只信 server 判定 ----
{
  const denied = fakeApi({
    permission: async () => ok({ allow: false, reason: "非开发白名单" }),
  });
  const out = await createPrimitives(denied.api).prepare_task_link({ session_id: "s1", qq_id: "x" });
  assert.equal(out, "错误: 非开发白名单");

  const { api, calls } = fakeApi();
  await createPrimitives(api).append_instruction({ session_id: "s1", qq_id: "dev1" });
  assert.deepEqual(calls.find((c) => c.fn === "permission")?.args, ["append", "dev1", "s1"]);
}

// ---- transition_task 不带 qq_id（旧语义：内部里程碑迁移）----
{
  const { api, calls } = fakeApi();
  await createPrimitives(api).transition_task({ session_id: "s1", status: "planning" });
  assert.deepEqual(calls.find((c) => c.fn === "transition")?.args, ["s1", "planning", undefined]);
}

// ---- list_events / check_permission 透传默认值 ----
{
  const { api, calls } = fakeApi();
  const prims = createPrimitives(api);
  await prims.list_events({ session_id: "s1" });
  assert.deepEqual(calls.find((c) => c.fn === "listEvents")?.args, ["s1", 0, 50]);
  const perm = JSON.parse(await prims.check_permission({ op: "read", qq_id: "dev1", session_id: "s1" }));
  assert.deepEqual(perm, { allow: true, reason: null });
}

// ---- run 调度原语（B7）----
{
  const { api, calls } = fakeApi();
  const prims = createPrimitives(api);
  const started = JSON.parse(await prims.start_run({ session_id: "s1", stage: "plan" }));
  assert.equal(started.run_id, "s1:plan:r1");
  assert.deepEqual(calls.find((c) => c.fn === "startRun")?.args, ["s1", "plan"]);

  const finished = JSON.parse(await prims.finish_run({ run_id: "r1", status: "succeeded", executor_ref: "t1" }));
  assert.equal(finished.status, "succeeded");
  assert.deepEqual(calls.find((c) => c.fn === "setRunStatus")?.args, ["r1", "succeeded", "t1", undefined]);

  const cancelled = JSON.parse(await prims.cancel_run({ session_id: "s1" }));
  assert.equal(cancelled.status, "cancelled");
  assert.equal(calls.filter((c) => c.fn === "setRunStatus").length, 2);
}

console.log("✅ test-dev-bridge 通过");
