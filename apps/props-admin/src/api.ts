const TOKEN_KEY = "propsAdminToken";

export function getAdminToken(): string {
  return localStorage.getItem(TOKEN_KEY) ?? "";
}

export function setAdminToken(token: string): void {
  if (token) localStorage.setItem(TOKEN_KEY, token);
  else localStorage.removeItem(TOKEN_KEY);
}

async function request<T>(path: string, init: RequestInit = {}): Promise<T> {
  const headers = new Headers(init.headers);
  headers.set("content-type", "application/json");
  const token = getAdminToken();
  if (token) headers.set("x-props-admin-token", token);
  const res = await fetch(`/internal/gameprops${path}`, { ...init, headers });
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

export const api = {
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
  importHeroes: () =>
    request<{ imported: { generals: number; skills: number }; source: string }>(
      "/import/heroes",
      { method: "POST" },
    ),
};
