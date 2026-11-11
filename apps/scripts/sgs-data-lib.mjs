// 三国杀脚本共用数据读取（P4.4.2 harness；S6 起事实源 = v2）。
//
// 供 `sgs-batch-status.mjs` / `sgs-batch-scaffold.mjs` / `check-sgs-batch-consistency.mjs` 复用：
// - `generals_generated.rs`：已实现（live）武将（v2 生成物）
// - `engine-catalog.json` 武将元数据（生成物）
// - `development-plan.md` 批次表
// - v2 技能注册表（`src/skills/*.rs` 的 `code`/`name` 字面量）

import { readdirSync, readFileSync } from "node:fs";
import { loadEngineCatalog } from "./lib/sgs-catalog.mjs";

export const PATHS = {
  skillsDir: "crates/engines/sanguosha2/src/skills",
  generalsRs: "crates/engines/sanguosha2/src/generals_generated.rs",
  catalogJson: "docs/games/sanguosha/data/engine-catalog.json",
  planMd: "docs/games/sanguosha/development-plan.md",
};

export const PACK_LABEL = {
  standard: "标准",
  wind_fire_forest_mountain: "风林火山",
  yijiang: "一将成名",
  jiexian: "界限突破",
};

export function readFile(path) {
  return readFileSync(path, "utf8");
}

/** 名称去重口径（与 v2 生成物一致）：去掉「界/将」前缀后同名视为同一武将。 */
const normName = (name) => String(name ?? "").replace(/^(界|将)/, "");

/**
 * v2 `generals_generated.rs`：live 武将 → gid → 技能 code 列表（技能名经 v2 注册表映射）。
 *
 * 注意：v2 生成物只保留「界/将」版（同名原版是 `Offline`），而 `development-plan.md` 的批次表
 * 用的是原版 gid → 这里按**名称去重**把原版 gid 也视为已实现（否则 Y1–Y6 会误报未实现）。
 */
export function loadReadyGenerals() {
  const text = readFile(PATHS.generalsRs);
  const nameToCode = loadSkillNameCodes();
  const ready = new Map();
  const liveNorm = new Set();
  const entryRe = /GeneralDef \{([\s\S]*?)\n    \},/g;
  for (const m of text.matchAll(entryRe)) {
    const body = m[1];
    const gid = Number(/gid:\s*(\d+)/.exec(body)?.[1] ?? 0);
    const name = /name:\s*"([^"]+)"/.exec(body)?.[1] ?? "";
    const status = /status:\s*Status::(\w+)/.exec(body)?.[1];
    if (!gid || status !== "Live") continue;
    liveNorm.add(normName(name));
    const skillsText = /skills:\s*&\[([^\]]*)\]/.exec(body)?.[1] ?? "";
    const skills = [...skillsText.matchAll(/"([^"]+)"/g)]
      .map((x) => nameToCode.get(x[1]))
      .filter(Boolean);
    ready.set(gid, skills);
  }
  // 同名原版 gid 也算已实现（功能由界/将版提供）。
  for (const g of loadGenerals().values()) {
    if (!ready.has(g.gid) && liveNorm.has(normName(g.name))) ready.set(g.gid, []);
  }
  return ready;
}

/** engine-catalog.json：gid → { code, gid, name, pack, kingdom, max_hp, gender, skills[] }。 */
export function loadGenerals() {
  const out = new Map();
  for (const g of loadEngineCatalog().generals) {
    out.set(g.gid, {
      code: g.code,
      gid: g.gid,
      name: g.name,
      pack: g.pack,
      kingdom: g.kingdom,
      max_hp: g.max_hp,
      gender: g.gender,
      is_lord: g.is_lord,
      skills: g.skills,
    });
  }
  return out;
}

/**
 * v2 技能注册表：技能中文名 → code（扫描 `src/skills/*.rs` 的 `code`/`name` 字面量）。
 * 供批次脚手架提示「同名技能复用 code」。
 */
export function loadSkillNameCodes() {
  const out = new Map();
  let files = [];
  try {
    files = readdirSync(PATHS.skillsDir).filter((f) => f.endsWith(".rs"));
  } catch {
    return out;
  }
  // SkillDef 字面量有两种排版（`code`/`name` 同行或分行）→ 按块提取。
  const codeRe = /code:\s*"([a-z0-9_]+)"/;
  const nameRe = /name:\s*"([^"]+)"/;
  for (const f of files) {
    const text = readFile(`${PATHS.skillsDir}/${f}`);
    for (const chunk of text.split("SkillDef {").slice(1)) {
      const block = chunk.slice(0, chunk.indexOf("};"));
      const code = codeRe.exec(block)?.[1];
      const name = nameRe.exec(block)?.[1];
      if (code && name && !out.has(name)) out.set(name, code);
    }
  }
  return out;
}

/** development-plan.md：批次表行（`| Y3 | 187 王异、... |` 或 `| J1 | 26 界黄忠、... |`）。 */
export function loadBatches() {
  const text = readFile(PATHS.planMd);
  const batches = [];
  for (const line of text.split(/\r?\n/)) {
    const m = /^\|\s*([YJ]\d+)(\s*✅)?\s*\|\s*(.+?)\s*\|\s*$/.exec(line);
    if (!m) continue;
    const entries = [];
    for (const e of m[3].matchAll(/(\d{1,3})\s+([^、|（(]+)/g)) {
      entries.push({ gid: Number(e[1]), name: e[2].trim() });
    }
    if (entries.length === 0) continue;
    batches.push({
      batch: m[1],
      pack: m[1].startsWith("J") ? "jiexian" : "yijiang",
      done: Boolean(m[2]),
      entries,
    });
  }
  return batches;
}

/** 兼容保留：v1 `p67` 断言值（v1 删除后恒为 null；S6 起不再参与门禁）。 */
export function loadP67Count() {
  return null;
}

/** 批次完成度报告 + 每组下一批（`Y` = 一将成名 / `J` = 界限突破）。 */
export function batchReport() {
  const ready = loadReadyGenerals();
  const batches = loadBatches();
  const report = batches.map((b) => {
    const done = b.entries.filter((e) => ready.has(e.gid));
    const missing = b.entries.filter((e) => !ready.has(e.gid));
    return { ...b, doneCount: done.length, total: b.entries.length, missing };
  });
  const nextOf = (prefix) =>
    report.find((b) => b.batch.startsWith(prefix) && b.doneCount < b.total) ?? null;
  return {
    ready,
    report,
    next: nextOf("Y"),
    nextJiexian: nextOf("J"),
  };
}
