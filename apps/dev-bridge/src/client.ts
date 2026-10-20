//! dev 内部 API 客户端（薄桥 → server `/internal/dev/*`）。
//!
//! 约束（ADR-0015 §2.3）：桥**不直连 SQLite**，一切状态/校验/持久化都经 server；
//! 桥只负责「原语语义 ↔ HTTP」的翻译与 `错误: ...` 文本格式。

export interface ApiResult<T = unknown> {
  ok: boolean;
  status: number;
  data: T | null;
  error: string | null;
}

export interface DevTaskView {
  session_id: string;
  group_id: string;
  qq_id: string;
  title: string | null;
  plan_path: string | null;
  status: string;
  created_at: string;
  updated_at: string;
  stage?: string | null;
  plan_format?: string | null;
}

export interface DevApiOptions {
  baseUrl?: string;
  fetchImpl?: typeof fetch;
}

export class DevApi {
  private baseUrl: string;
  private fetchImpl: typeof fetch;

  constructor(opts: DevApiOptions = {}) {
    this.baseUrl = (opts.baseUrl ?? process.env.SERVER_URL ?? "http://127.0.0.1:3000").replace(/\/$/, "");
    this.fetchImpl = opts.fetchImpl ?? fetch;
  }

  private async call<T>(
    method: "GET" | "POST",
    path: string,
    body?: unknown,
  ): Promise<ApiResult<T>> {
    let res: Response;
    try {
      res = await this.fetchImpl(`${this.baseUrl}${path}`, {
        method,
        headers: body === undefined ? undefined : { "content-type": "application/json" },
        body: body === undefined ? undefined : JSON.stringify(body),
      });
    } catch (e) {
      return { ok: false, status: 0, data: null, error: `server 不可达: ${(e as Error).message}` };
    }
    const text = await res.text();
    let data: any = null;
    if (text) {
      try {
        data = JSON.parse(text);
      } catch {
        data = text;
      }
    }
    if (!res.ok) {
      const err = (data && typeof data === "object" && (data.error ?? data.reason)) || `HTTP ${res.status}`;
      return { ok: false, status: res.status, data: null, error: String(err) };
    }
    return { ok: true, status: res.status, data: data as T, error: null };
  }

  listTasks(status?: string): Promise<ApiResult<DevTaskView[]>> {
    const q = status ? `?status=${encodeURIComponent(status)}` : "";
    return this.call("GET", `/internal/dev/tasks${q}`);
  }

  /** 单任务详情：server 返回 `{task, events, artifacts}`，这里取 `task`（与原语契约一致）。 */
  async getTask(sessionId: string): Promise<ApiResult<DevTaskView>> {
    const r = await this.call<any>("GET", `/internal/dev_task/${encodeURIComponent(sessionId)}`);
    if (!r.ok) return r as ApiResult<DevTaskView>;
    return { ...r, data: (r.data?.task ?? r.data) as DevTaskView };
  }

  createTask(sessionId: string, groupId: string, qqId: string): Promise<ApiResult<DevTaskView>> {
    return this.call("POST", "/internal/dev/tasks", {
      session_id: sessionId,
      group_id: groupId,
      qq_id: qqId,
    });
  }

  transition(
    sessionId: string,
    to: string,
    qqId?: string,
  ): Promise<ApiResult<DevTaskView>> {
    return this.call("POST", "/internal/dev/transition", {
      session_id: sessionId,
      to,
      ...(qqId ? { qq_id: qqId } : {}),
    });
  }

  confirm(sessionId: string, qqId: string): Promise<ApiResult<DevTaskView>> {
    return this.call("POST", "/internal/dev/confirm", { session_id: sessionId, qq_id: qqId });
  }

  updateMeta(
    sessionId: string,
    meta: { title?: string; plan_path?: string; plan_format?: string },
  ): Promise<ApiResult<DevTaskView>> {
    return this.call("POST", "/internal/dev/meta", { session_id: sessionId, ...meta });
  }

  permission(op: string, qqId: string, sessionId?: string): Promise<ApiResult<{ allow: boolean; reason: string | null }>> {
    return this.call("POST", "/internal/dev/permission", {
      op,
      qq_id: qqId,
      ...(sessionId ? { session_id: sessionId } : {}),
    });
  }


  startRun(
    sessionId: string,
    stage: string,
    opts: { branch?: string; commitSha?: string; idempotencyKey?: string } = {},
  ): Promise<ApiResult<{ run_id: string; status: string }>> {
    return this.call("POST", "/internal/dev/runs", {
      session_id: sessionId,
      stage,
      executor: "langgraph",
      ...(opts.branch ? { branch: opts.branch } : {}),
      ...(opts.commitSha ? { commit_sha: opts.commitSha } : {}),
      ...(opts.idempotencyKey ? { idempotency_key: opts.idempotencyKey } : {}),
    });
  }

  setRunStatus(
    runId: string,
    status: string,
    ref?: string,
    lastError?: string,
  ): Promise<ApiResult<{ run_id: string; status: string }>> {
    return this.call("POST", `/internal/dev/runs/${encodeURIComponent(runId)}/status`, {
      status,
      ...(ref ? { executor_ref: ref } : {}),
      ...(lastError ? { last_error: lastError } : {}),
    });
  }

  activeRun(sessionId: string): Promise<ApiResult<{ run_id: string; status: string; stage: string }>> {
    return this.call("GET", `/internal/dev/runs/${encodeURIComponent(sessionId)}/active`);
  }

  notify(sessionId: string, text: string, kind?: string): Promise<ApiResult<{ event_id: string; inserted: boolean }>> {
    return this.call("POST", "/internal/dev/notify", {
      session_id: sessionId,
      text,
      ...(kind ? { kind } : {}),
    });
  }

  listEvents(sessionId: string, sinceId = 0, limit = 50): Promise<ApiResult<unknown[]>> {
    const q = `session_id=${encodeURIComponent(sessionId)}&since_id=${sinceId}&limit=${limit}`;
    return this.call("GET", `/internal/dev/events?${q}`);
  }
}

export function errorText(r: { error: string | null; status: number }): string {
  return `错误: ${r.error ?? `HTTP ${r.status}`}`;
}
