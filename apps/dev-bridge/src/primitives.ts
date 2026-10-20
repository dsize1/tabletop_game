//! dev 原语实现（与已删除的 `dev-task-mcp` 契约对齐）。
//!
//! 输出约定（契约测试锁定）：
//! - 成功：JSON 文本（与旧 MCP 同构）；
//! - 失败：`错误: <reason>`（与旧实现同一前缀/风格）；
//! - 能力描述符 schema 不变（`name/rw/description/usage/example`）。
//!
//! 本批实现：11 个旧原语 + v2 的 `approve_plan` / `approve_verify` / `list_events` / `check_permission`。
//! `start_run` / `finish_run` / `cancel_run`（B7）与 `notify_user`（B4）在各自批次补。

import { DevApi, errorText, type ApiResult, type DevTaskView } from "./client.js";
import { readPlanProgress } from "./plan.js";

export interface PrimitiveArgs {
  session_id?: string;
  group_id?: string;
  qq_id?: string;
  status?: string;
  title?: string;
  plan_path?: string;
  plan_format?: string;
  event_id?: string;
  since_id?: number;
  limit?: number;
  op?: string;
  stage?: string;
  run_id?: string;
  executor_ref?: string;
  last_error?: string;
}

/** 能力描述符（旧口径，逐字对齐已删除的 dev-task-mcp；顺序/文案变更会破坏 `#能力` 契约）。 */
export const CAPABILITIES = [
  { name: "list_tasks", rw: "读", description: "列出所有开发任务（可按状态过滤）", usage: "list_tasks(status?)", example: "#任务 或 #任务 running" },
  { name: "get_task", rw: "读", description: "查询单个任务详情", usage: "get_task(session_id)", example: "（网关内部；用户用 #进度 <sessionId>）" },
  { name: "get_progress", rw: "读", description: "查询任务进度（阶段 + 待办/验证完成度）", usage: "get_progress(session_id)", example: "#进度 <sessionId>" },
  { name: "prepare_task_link", rw: "读", description: "校验归属+非终态，返回计划短链资源（token 由网关签发）", usage: "prepare_task_link(session_id, qq_id)", example: "（网关内部；#进度 自动重签短链）" },
  { name: "append_instruction", rw: "写", description: "校验归属+非终态，授权把自由文本追加到已有子会话", usage: "append_instruction(session_id, qq_id)", example: "#补充 <sessionId> <正文>" },
  { name: "list_capabilities", rw: "读", description: "列出本服务全部 MCP 能力", usage: "list_capabilities()", example: "#能力" },
  { name: "create_task", rw: "写", description: "创建开发任务（白名单校验）", usage: "create_task(session_id, group_id, qq_id)", example: "#发起 <需求>（#开发 为别名）" },
  { name: "confirm_task", rw: "写", description: "人工确认计划并开始执行（白名单+归属校验）", usage: "confirm_task(session_id, qq_id)", example: "#确认 <sessionId>" },
  { name: "cancel_task", rw: "写", description: "人工取消任务（白名单+归属校验）", usage: "cancel_task(session_id, qq_id)", example: "#取消 <sessionId>" },
  { name: "transition_task", rw: "写", description: "任务状态迁移（内容里程碑，确定性迁移表校验）", usage: "transition_task(session_id, status)", example: "（网关内部；子会话里程碑驱动）" },
  { name: "update_task_meta", rw: "写", description: "回填任务标题/计划路径", usage: "update_task_meta(session_id, title?, plan_path?)", example: "（网关内部回填）" },
];

function need(args: PrimitiveArgs, key: keyof PrimitiveArgs): string {
  const v = args[key];
  if (v === undefined || v === null || String(v).trim() === "") {
    throw new Error(`缺少参数 ${String(key)}`);
  }
  return String(v);
}

async function failOr<T>(r: ApiResult<T>, fn: (data: T) => unknown): Promise<string> {
  if (!r.ok) return errorText(r);
  return JSON.stringify(fn(r.data as T));
}

export function createPrimitives(api: DevApi) {
  return {
    async list_tasks(args: PrimitiveArgs): Promise<string> {
      const r = await api.listTasks(args.status);
      return failOr(r, (d) => d);
    },

    async get_task(args: PrimitiveArgs): Promise<string> {
      const sid = need(args, "session_id");
      const r = await api.getTask(sid);
      return failOr(r, (d) => d);
    },

    async get_progress(args: PrimitiveArgs): Promise<string> {
      const sid = need(args, "session_id");
      const r = await api.getTask(sid);
      if (!r.ok || !r.data) return errorText(r);
      const t = r.data as DevTaskView;
      const progress = readPlanProgress(t.plan_path);
      return JSON.stringify({
        session_id: t.session_id,
        status: t.status,
        title: t.title,
        plan_path: t.plan_path,
        todo: progress.todo,
        verify: progress.verify,
      });
    },

    async prepare_task_link(args: PrimitiveArgs): Promise<string> {
      const sid = need(args, "session_id");
      const qq = need(args, "qq_id");
      const perm = await api.permission("read", qq, sid);
      if (!perm.ok) return errorText(perm);
      if (!perm.data?.allow) return `错误: ${perm.data?.reason ?? "无权限"}`;
      const r = await api.getTask(sid);
      if (!r.ok || !r.data) return errorText(r);
      const t = r.data as DevTaskView;
      return JSON.stringify({
        session_id: t.session_id,
        plan_path: t.plan_path,
        resource: `dev_task:${t.session_id}`,
      });
    },

    async append_instruction(args: PrimitiveArgs): Promise<string> {
      const sid = need(args, "session_id");
      const qq = need(args, "qq_id");
      const perm = await api.permission("append", qq, sid);
      if (!perm.ok) return errorText(perm);
      if (!perm.data?.allow) return `错误: ${perm.data?.reason ?? "无权限"}`;
      const r = await api.getTask(sid);
      if (!r.ok || !r.data) return errorText(r);
      const t = r.data as DevTaskView;
      return JSON.stringify({ session_id: t.session_id, status: t.status, plan_path: t.plan_path });
    },

    async list_capabilities(): Promise<string> {
      return JSON.stringify(CAPABILITIES);
    },

    async create_task(args: PrimitiveArgs): Promise<string> {
      const sid = need(args, "session_id");
      const gid = need(args, "group_id");
      const qq = need(args, "qq_id");
      const r = await api.createTask(sid, gid, qq);
      return failOr(r, (d) => d);
    },

    async confirm_task(args: PrimitiveArgs): Promise<string> {
      const sid = need(args, "session_id");
      const qq = need(args, "qq_id");
      const r = await api.confirm(sid, qq);
      return failOr(r, (d) => d);
    },

    async cancel_task(args: PrimitiveArgs): Promise<string> {
      const sid = need(args, "session_id");
      const qq = need(args, "qq_id");
      const r = await api.transition(sid, "cancelled", qq);
      return failOr(r, (d) => d);
    },

    async transition_task(args: PrimitiveArgs): Promise<string> {
      const sid = need(args, "session_id");
      const to = need(args, "status");
      // 旧语义：里程碑迁移由内部驱动，不带归属校验（server 仍校验迁移表）。
      const r = await api.transition(sid, to);
      return failOr(r, (d) => d);
    },

    async update_task_meta(args: PrimitiveArgs): Promise<string> {
      const sid = need(args, "session_id");
      const r = await api.updateMeta(sid, {
        ...(args.title !== undefined ? { title: args.title } : {}),
        ...(args.plan_path !== undefined ? { plan_path: args.plan_path } : {}),
        ...(args.plan_format !== undefined ? { plan_format: args.plan_format } : {}),
      });
      return failOr(r, (d) => d);
    },

    // ---- v2（ADR-0015 §4：审批拆两个语义）----

    async approve_plan(args: PrimitiveArgs): Promise<string> {
      return approveByStage(api, args, "awaiting_confirm");
    },

    async approve_verify(args: PrimitiveArgs): Promise<string> {
      return approveByStage(api, args, "verifying");
    },


    // ---- v2（B7：run 调度；映射 dev_runs）----

    async start_run(args: PrimitiveArgs): Promise<string> {
      const sid = need(args, "session_id");
      const stage = need(args, "stage");
      const r = await api.startRun(sid, stage);
      return failOr(r, (d) => d);
    },

    async finish_run(args: PrimitiveArgs): Promise<string> {
      const runId = need(args, "run_id");
      const status = (args.status ?? "succeeded").trim();
      const r = await api.setRunStatus(runId, status, args.executor_ref, args.last_error);
      return failOr(r, (d) => d);
    },

    async cancel_run(args: PrimitiveArgs): Promise<string> {
      const sid = need(args, "session_id");
      const active = await api.activeRun(sid);
      if (!active.ok || !active.data) return errorText(active);
      const r = await api.setRunStatus(active.data.run_id, "cancelled");
      return failOr(r, (d) => d);
    },

    async list_events(args: PrimitiveArgs): Promise<string> {
      const sid = need(args, "session_id");
      const r = await api.listEvents(sid, args.since_id ?? 0, args.limit ?? 50);
      return failOr(r, (d) => d);
    },

    async check_permission(args: PrimitiveArgs): Promise<string> {
      const op = need(args, "op");
      const qq = need(args, "qq_id");
      const r = await api.permission(op, qq, args.session_id);
      if (!r.ok) return errorText(r);
      return JSON.stringify(r.data);
    },
  };
}

async function approveByStage(
  api: DevApi,
  args: PrimitiveArgs,
  expected: string,
): Promise<string> {
  const sid = need(args, "session_id");
  const qq = need(args, "qq_id");
  const cur = await api.getTask(sid);
  if (!cur.ok || !cur.data) return errorText(cur);
  const status = (cur.data as DevTaskView).status;
  if (status !== expected) {
    return `错误: 非法状态迁移: ${status} -> ${expected === "awaiting_confirm" ? "confirmed(plan)" : "confirmed(verify)"}`;
  }
  const r = await api.confirm(sid, qq);
  return failOr(r, (d) => d);
}

export type PrimitiveTable = ReturnType<typeof createPrimitives>;
