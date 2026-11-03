const TOKEN_KEY = "propsAdminToken";

export function getAdminToken(): string {
  return localStorage.getItem(TOKEN_KEY) ?? "";
}

export function setAdminToken(token: string): void {
  if (token) localStorage.setItem(TOKEN_KEY, token);
  else localStorage.removeItem(TOKEN_KEY);
}

async function fetchJson<T>(url: string, init: RequestInit = {}): Promise<T> {
  const headers = new Headers(init.headers);
  headers.set("content-type", "application/json");
  const token = getAdminToken();
  if (token) headers.set("x-props-admin-token", token);
  const res = await fetch(url, { ...init, headers });
  const text = await res.text();
  let body: unknown = null;
  try {
    body = text ? JSON.parse(text) : null;
  } catch {
    body = { error: text };
  }
  if (!res.ok) {
    const msg = (body as { error?: string })?.error ?? `HTTP ${res.status}`;
    throw new Error(msg);
  }
  return body as T;
}

async function request<T>(path: string, init: RequestInit = {}): Promise<T> {
  return fetchJson<T>(`/internal/gameprops${path}`, init);
}

export const api = {
  session: () => request<PropsSession>("/session"),
  summary: () => request<Record<string, unknown>>("/summary"),
  list: (
    table: string,
    params: { q?: string; status?: string; limit?: number; offset?: number } = {},
  ) => {
    const sp = new URLSearchParams();
    if (params.q) sp.set("q", params.q);
    if (params.status) sp.set("status", params.status);
    sp.set("limit", String(params.limit ?? 200));
    sp.set("offset", String(params.offset ?? 0));
    return request<{ table: string; rows: Record<string, unknown>[] }>(
      `/${table}?${sp.toString()}`,
    );
  },
  upsert: (table: string, row: Record<string, unknown>) =>
    request<{ row: Record<string, unknown> }>(`/${table}`, {
      method: "POST",
      body: JSON.stringify(row),
    }),
  setStatus: (table: string, entity_code: string, to_status: string, note?: string) =>
    request<{ row: Record<string, unknown> }>("/status", {
      method: "POST",
      body: JSON.stringify({ table, entity_code, to_status, note }),
    }),
  seed: () =>
    request<{ seeded: Record<string, number> }>("/seed", { method: "POST" }),
  importEngine: () =>
    request<{
      imported: {
        card_defs: number;
        cards: number;
        generals: number;
        skills: number;
        general_skills: number;
      };
      source: string;
    }>("/import/engine", { method: "POST" }),
  importHeroes: () =>
    request<{ imported: { generals: number; skills: number }; source: string }>(
      "/import/heroes",
      { method: "POST" },
    ),
};

// ---------------------------------------------------------------------------
// 工作台（P5 基座）：模板 / 预览 / 决策日志
// ---------------------------------------------------------------------------

export interface TemplateRow {
  id: number;
  game_id: string;
  name: string;
  source: string;
  meta?: string | null;
  created_by?: string | null;
  created_at: string;
  updated_at: string;
}

export type TemplateDetail = Omit<TemplateRow, "meta"> & {
  snapshot: unknown;
  meta: unknown;
};

export interface PreviewStep {
  seq: number;
  actor: string;
  source: string;
  action: string;
  message: string;
  finished: boolean;
  legal?: Array<{ id: string; description: string }>;
  candidates?: Array<{ id: string; description: string }>;
  score?: number | null;
  priority?: number | null;
  reason?: string | null;
  state_digest?: string | null;
  latency_ms?: number | null;
  events?: unknown[];
}

export interface PreviewSeat {
  index: number;
  player_id: string;
  general?: string | null;
  general_name?: string;
  identity?: string;
  hp?: number;
  max_hp?: number;
  alive?: boolean;
  is_bot?: boolean;
}

export interface PreviewState {
  preview_id: string;
  room_id?: string;
  game_id?: string;
  viewer?: string | null;
  public?: { data?: unknown };
  player?: { data?: unknown };
  actions?: Array<{ id: string; description: string }>;
  names?: Record<string, string>;
  finished?: boolean;
  summary?: string;
  seats?: PreviewSeat[] | { seats?: PreviewSeat[] };
  steps?: PreviewStep[];
  raw?: unknown;
  template_id?: number | null;
}

export interface PropsSession {
  openid: string;
  scope: "admin" | "workbench";
  menus: string[];
}

export interface SandboxGeneral {
  code: string;
  name: string;
  kingdom: string;
  pack: string;
  max_hp: number;
  is_lord: boolean;
  skills: string[];
}

export interface SandboxMeta {
  players: number[];
  generals: SandboxGeneral[];
  identities: Record<string, Record<string, number>>;
  packs: { cards: string[]; general_packs: string[] };
}

export interface SandboxSeatInput {
  index: number;
  player_id?: string;
  general?: string;
  identity?: string;
}

export interface DecisionLogRow {
  id: number;
  seq: number;
  player_id: string;
  source: string;
  chosen?: string | null;
  confidence?: number | null;
  latency_ms?: number | null;
  reason?: string | null;
  state_digest?: string | null;
  created_at: string;
  legal_actions?: string;
  candidates?: string | null;
}

export function templateScene(meta: unknown): unknown {
  if (!meta || typeof meta !== "object") return null;
  return (meta as { scene?: unknown }).scene ?? null;
}

export const workbenchApi = {
  templates: (gameId = "sanguosha") =>
    request<{ rows: TemplateRow[]; total: number }>(
      `/templates?game_id=${encodeURIComponent(gameId)}`,
    ),
  template: (id: number) => request<TemplateDetail>(`/templates/${id}`),
  createTemplate: (body: {
    game_id: string;
    name: string;
    source?: string;
    snapshot: unknown;
    meta?: unknown;
  }) => request<{ row: TemplateRow }>(`/templates`, { method: "POST", body: JSON.stringify(body) }),
  updateTemplate: (
    id: number,
    body: { name?: string; snapshot?: unknown; meta?: unknown },
  ) => request<{ row: TemplateRow }>(`/templates/${id}`, { method: "PUT", body: JSON.stringify(body) }),
  duplicateTemplate: (id: number, name?: string) =>
    request<{ row: TemplateRow }>(`/templates/${id}/duplicate`, {
      method: "POST",
      body: JSON.stringify({ name }),
    }),
  deleteTemplate: (id: number) =>
    request<{ ok: boolean; existed: boolean }>(`/templates/${id}`, { method: "DELETE" }),
  fromRoom: (body: { room_id: string; name?: string; game_id?: string; scene?: unknown }) =>
    request<{ row: TemplateRow }>(`/templates/from-room`, {
      method: "POST",
      body: JSON.stringify(body),
    }),
  fromReplay: (roomId: string, name?: string) =>
    request<{ row: TemplateRow }>(`/templates/from-replay`, {
      method: "POST",
      body: JSON.stringify({ room_id: roomId, name }),
    }),
};

async function previewRequest<T>(path: string, init: RequestInit = {}): Promise<T> {
  return fetchJson<T>(`/internal/game/preview${path}`, init);
}

export const previewApi = {
  create: (body: {
    template_id?: number;
    snapshot?: unknown;
    scene?: unknown;
    viewer?: string;
    game_id?: string;
  }) =>
    previewRequest<PreviewState>("", { method: "POST", body: JSON.stringify(body) }),
  get: (id: string, viewer?: string, raw = true) =>
    previewRequest<PreviewState>(
      `/${encodeURIComponent(id)}?raw=${raw ? 1 : 0}` +
        (viewer ? `&viewer=${encodeURIComponent(viewer)}&actor=${encodeURIComponent(viewer)}` : ""),
    ),
  action: (id: string, playerId: string, action: string) =>
    previewRequest<{ ok: boolean; step: PreviewStep; view: PreviewState }>(
      `/${encodeURIComponent(id)}/action`,
      { method: "POST", body: JSON.stringify({ player_id: playerId, action }) },
    ),
  drive: (
    id: string,
    body: { max_steps?: number; policy?: string; actor?: string; viewer?: string },
  ) =>
    previewRequest<{
      ok: boolean;
      steps: number;
      stop?: string;
      errors?: string[];
      trace?: PreviewStep;
      view: PreviewState;
    }>(`/${encodeURIComponent(id)}/drive`, { method: "POST", body: JSON.stringify(body) }),
  reset: (id: string) =>
    previewRequest<{ ok: boolean; view: PreviewState }>(`/${encodeURIComponent(id)}/reset`, {
      method: "POST",
      body: "{}",
    }),
  remove: (id: string) =>
    previewRequest<{ ok: boolean; existed: boolean }>(`/${encodeURIComponent(id)}`, {
      method: "DELETE",
    }),
  meta: () => previewRequest<SandboxMeta>("/meta"),
  sandbox: (body: {
    players: number;
    seed?: number | null;
    seats: SandboxSeatInput[];
    save_template?: boolean;
    name?: string;
  }) =>
    previewRequest<PreviewState & { seed: number }>("/sandbox", {
      method: "POST",
      body: JSON.stringify(body),
    }),
  validateScene: (snapshot: unknown, scene: unknown) =>
    previewRequest<{ ok: boolean; snapshot: unknown; seat_count: number }>("/scene", {
      method: "POST",
      body: JSON.stringify({ snapshot, scene }),
    }),
};

export const logApi = {
  list: (params: { game_id?: string; room_id: string; since_seq?: number; limit?: number }) => {
    const sp = new URLSearchParams();
    sp.set("game_id", params.game_id ?? "sanguosha");
    sp.set("room_id", params.room_id);
    if (params.since_seq) sp.set("since_seq", String(params.since_seq));
    sp.set("limit", String(params.limit ?? 100));
    return request<{ rows: DecisionLogRow[] }>(`/decision-logs?${sp.toString()}`);
  },
};
