#!/usr/bin/env node
// 全武将验证审计（机器证据；不判语义、不产「通过」）。
//
// 边界（memory_palace/plan/next-general-audit.md §4.0）：
// - 只产出**机械证据**：合法动作/待办项可达、执行不报错、牌数守恒、pending 可继续、同 seed 可复现；
// - 「语义是否与原技能一致」必须由人工在工作台走查后写入 `docs/games/sanguosha/general-audit.md`；
// - 不写 `game_state` / `game_events` / `game_decision_log`（只走内存预览会话，且审计后校验行数不变）。
//
// 用法：
//   node apps/scripts/sgs-general-audit.mjs --list                     # 名录 + 批次表自检
//   node apps/scripts/sgs-general-audit.mjs --batch B0 --repeat 2      # 跑一批 → data/audit/B0.json
//   node apps/scripts/sgs-general-audit.mjs --general 曹仁
//   node apps/scripts/sgs-general-audit.mjs --check --sample 8         # 门禁抽样（失败非 0 退出）
//   node apps/scripts/sgs-general-audit.mjs --all --out data/audit/all.json
//   node apps/scripts/sgs-general-audit.mjs --general 卢植 --no-scene   # 保留开局 pending（开局技）
//
// 依赖：Redis（auth）、已构建 server（cargo build -p server）。缺失 → skip 0。

import { spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync } from "node:fs";
import fs from "node:fs/promises";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import { DatabaseSync } from "node:sqlite";
import { loadCatalog, projectRoot } from "./lib/sgs-catalog.mjs";
import { BATCHES, expandBatches, findBatch } from "./lib/sgs-batches.mjs";
import { mergeScenarios } from "./lib/sgs-scenes.mjs";

const args = process.argv.slice(2);
const flag = (name) => args.includes(name);
const opt = (name, fallback = undefined) => {
  const i = args.indexOf(name);
  if (i < 0) return fallback;
  const v = args[i + 1];
  return v && !v.startsWith("--") ? v : true;
};

const PORT = Number(process.env.SGS_AUDIT_PORT ?? 3198);
const SERVER_URL = `http://127.0.0.1:${PORT}`;
const REDIS_URL = process.env.REDIS_URL ?? "redis://127.0.0.1:6379";
const TOKEN = "sgs-audit-token";
const ACTOR = "sgs-audit";
const SEED = Number(opt("--seed", 20261105));
const REPEAT = Math.max(1, Number(opt("--repeat", flag("--check") ? 2 : 1)) || 1);
const PLAYERS = Number(opt("--players", 5));
/** L1 走查步数：1 步 ≈ 一次 AI 动作；12 步约覆盖一轮（含目标下回合准备/判定/摸牌）。 */
const MAX_STEPS = Math.max(1, Number(opt("--max-steps", 12)) || 12);
/** `--no-scene`：不注入 L1 场景，直接从沙盒开局快照起走（用于开局技：明任/怀橘/宗祚/血裔/巧变/化身）。 */
const NO_SCENE = flag("--no-scene");
const SQLITE_PATH = path.join(os.tmpdir(), `sgs_audit_${Date.now()}.db`);
const OUT_DIR = path.join(projectRoot, "data/audit");
const MATRIX_FILE = path.join(projectRoot, "docs/games/sanguosha/general-audit.md");
const SCENARIOS_FILE = path.join(projectRoot, "docs/games/sanguosha/general-audit-scenarios.json");
const EVIDENCE_FILE = path.join(projectRoot, "docs/games/sanguosha/general-audit-evidence.md");

// L1 通用基线：目标座位固定手牌/装备；其他座位最小配置。
const L1_HAND = [
  "sha",
  "sha",
  "shan",
  "tao",
  "jiu",
  "juedou",
  "guohe_chaiqiao",
  "shunshou_qianyang",
  "wuxie_keji",
  // 装备手牌：直谏（张昭张弘）/ 解围（曹仁，装备区牌当无懈）等需要装备牌可达。
  "zhuge_liannu",
];
const L1_EQUIPS = { weapon: "zhuge_liannu", armor: "bagua_zhen" };
// 其他座位仅 2 张：把牌留在牌堆里，减少「场景取牌失败」与误触达。
const L1_OTHER_HAND = ["sha", "shan"];
const L1_DECK_TOP = ["sha", "shan", "tao", "wuxie_keji", "jiu", "juedou"];
const FILLER_GENERALS = ["界刘备", "界曹操", "界孙权", "界吕布"];

const children = [];
let failed = 0;
let infraFailed = false;

const log = (msg) => console.log(`[sgs-audit] ${msg}`);

function killTree(child) {
  try {
    if (process.platform === "win32" && child.pid) {
      spawnSync("taskkill", ["/F", "/T", "/PID", String(child.pid)], { stdio: "ignore" });
    } else {
      child.kill();
    }
  } catch {
    /* ignore */
  }
}
const cleanup = () => children.forEach(killTree);

function tcpOk(host, port, timeoutMs = 700) {
  return new Promise((resolve) => {
    const sock = net.connect({ host, port });
    const done = (ok) => {
      sock.destroy();
      resolve(ok);
    };
    sock.setTimeout(timeoutMs, () => done(false));
    sock.once("connect", () => done(true));
    sock.once("error", () => done(false));
  });
}
async function waitHealth(url, timeoutMs = 20000) {
  const end = Date.now() + timeoutMs;
  while (Date.now() < end) {
    try {
      const r = await fetch(url, { signal: AbortSignal.timeout(1500) });
      if (r.status < 500) return true;
    } catch {
      /* retry */
    }
    await new Promise((r) => setTimeout(r, 250));
  }
  return false;
}

async function api(pathname, { method = "GET", body, timeout = 25000 } = {}) {
  const r = await fetch(`${SERVER_URL}${pathname}`, {
    method,
    headers: {
      "content-type": "application/json",
      "x-props-admin-token": TOKEN,
      "x-props-actor": ACTOR,
    },
    body: body === undefined ? undefined : JSON.stringify(body),
    signal: AbortSignal.timeout(timeout),
  });
  const text = await r.text();
  let json = null;
  try {
    json = text ? JSON.parse(text) : null;
  } catch {
    json = { raw: text.slice(0, 2000) };
  }
  return { status: r.status, body: json };
}
function must(res, label) {
  if (res.status >= 200 && res.status < 300) return res.body;
  throw new Error(`${label} 失败（HTTP ${res.status}）：${JSON.stringify(res.body).slice(0, 600)}`);
}

function dbCounts() {
  const out = {};
  if (!existsSync(SQLITE_PATH)) return out;
  const db = new DatabaseSync(SQLITE_PATH);
  try {
    for (const t of ["game_state", "game_events", "game_decision_log", "game_state_templates"]) {
      try {
        out[t] = db.prepare(`SELECT COUNT(*) AS n FROM ${t}`).get().n;
      } catch {
        out[t] = null;
      }
    }
  } finally {
    db.close();
  }
  return out;
}

/** 牌数（所有区域合计；守恒断言）。v2：`Game.zone` 每张牌一条位置记录（含 Void）。 */
function countCards(st) {
  if (st?.zone && typeof st.zone === "object" && !Array.isArray(st.zone)) {
    return Object.keys(st.zone).length;
  }
  const n = (a) => (Array.isArray(a) ? a.length : 0);
  let total =
    n(st?.deck?.draw) + n(st?.deck?.discard) + n(st?.deck?.processing) + n(st?.deck?.void);
  for (const seat of st?.seats ?? []) {
    total += n(seat.hand) + n(seat.judge_zone);
    for (const id of Object.values(seat.equips ?? {})) if (id) total += 1;
    for (const list of Object.values(seat.marks ?? {})) total += n(list);
  }
  return total;
}

function canonicalDigest(value) {
  const sort = (v) => {
    if (Array.isArray(v)) return v.map(sort);
    if (v && typeof v === "object") {
      const o = {};
      for (const k of Object.keys(v).sort()) o[k] = sort(v[k]);
      return o;
    }
    return v;
  };
  return createHash("sha256").update(JSON.stringify(sort(value))).digest("hex").slice(0, 16);
}

/** L1 场景（目标座位：出牌阶段 + 固定手牌装备）。 */
function l1Scene(targetId) {
  return {
    round: 1,
    turn: 0,
    turn_phase: "play",
    phase: "turn",
    seed: SEED,
    deck_top: L1_DECK_TOP,
    // 先重置其他座位（原手牌进弃牌堆），再设置目标座位：
    // 场景按数组顺序应用，目标座位最后处理时可从弃牌堆取到刚被换下的牌
    // （否则同名同定义的备用副本可能仍握在其他座位手里 → “找不到可用的牌”）。
    seats: [
      ...Array.from({ length: PLAYERS - 1 }, (_, i) => ({ index: i + 1, hand: L1_OTHER_HAND })),
      {
        index: 0,
        player_id: targetId,
        hand: L1_HAND,
        equips: L1_EQUIPS,
        judge: [],
        chained: false,
        flipped: false,
      },
    ],
  };
}

/**
 * 技能可达性：动作 id 或 pending 引用该技能 code。
 * 覆盖 `skill:<code>[:...]`、`choose:<code>`、`use:<code>` 等既有 id 命名。
 */
function mentionedSkills(ids, skills) {
  const out = new Set();
  for (const id of ids) {
    for (const s of skills) {
      const re = new RegExp(`(^|[^a-z0-9_])${s.code}([^a-z0-9_]|$)`);
      if (id === s.code || re.test(id)) out.add(s.code);
    }
  }
  return out;
}

/** 单将审计（L1 走查）。返回证据对象（不含「通过」语义）。 */
async function auditGeneral(general) {
  const result = {
    code: general.code,
    name: general.name,
    pack: general.pack,
    isJiexian: general.isJiexian,
    skills: general.skills.map((s) => ({ code: s.code, name: s.name, kind: s.kind })),
    status: "阻塞",
    reason: "",
    runs: [],
    reachedSkills: [],
    untrackedSkills: [],
    invariants: {},
    seed: SEED,
  };

  const attempt = async () => {
    const sandbox = must(
      await api("/internal/game/preview/sandbox", {
        method: "POST",
        body: {
          players: PLAYERS,
          seed: SEED,
          seats: [
            { index: 0, general: general.code, identity: "lord" },
            ...FILLER_GENERALS.slice(0, PLAYERS - 1).map((_n, i) => ({ index: i + 1 })),
          ],
        },
      }),
      `sandbox(${general.name})`,
    );
    const raw0 = sandbox.raw;
    // v2：沙盒视图 `seats.seats[].player_id`（v1 是 `raw.data.seats[].player_id`）。
    const seatInfo = sandbox.seats?.seats?.[0];
    const targetId =
      seatInfo?.player_id ?? raw0?.data?.seats?.[0]?.player_id ?? raw0?.players?.[0];
    const targetSeat = Number(seatInfo?.index ?? 0);
    if (!targetId) throw new Error("sandbox 未返回座位 0 的 player_id");

    const created = must(
      await api("/internal/game/preview", {
        method: "POST",
        body: {
          game_id: "sanguosha",
          snapshot: raw0,
          ...(NO_SCENE ? {} : { scene: l1Scene(targetId) }),
          viewer: targetId,
        },
      }),
      `create+scene(${general.name})`,
    );
    const id = created.preview_id;
    const rawBefore = created.raw?.data ?? null;
    const run = {
      previewId: id,
      cardsBefore: countCards(rawBefore),
      cardsAfter: null,
      steps: [],
      reached: [],
      digests: [],
      autoPicks: [],
      driveErrors: [],
    };

    const seenIds = new Set();
    const reached = new Set();
    let cur = created;

    const collect = (sheet, tag) => {
      const ids = (sheet?.actions ?? []).map((a) => a.id);
      const st = sheet?.raw?.data ?? null;
      const rawPending = st?.pending ?? null;
      const pending = rawPending
        ? {
            // v2：raw 里的 `actor` 是座位下标；v1 是 player_id。两者都兼容。
            actor: rawPending.actor ?? rawPending.player_id ?? null,
            skill: rawPending.skill ?? null,
            reason: rawPending.data?.reason ?? null,
            required: rawPending.data?.required ?? null,
            options: (rawPending.options ?? []).map((o) => o.id).slice(0, 20),
          }
        : null;
      const pool = [...ids, ...(pending?.options ?? [])];
      const ownCodes = new Set(general.skills.map((s) => s.code));
      if (pending?.skill && ownCodes.has(pending.skill)) reached.add(pending.skill);
      // pending 的 data 也可能携带技能归因（如庸肆弃牌 `data.reason = "yongsi"`）。
      const dataText = pending ? JSON.stringify(rawPending?.data ?? {}) : "";
      const dataHits = dataText ? [...mentionedSkills([dataText], general.skills)] : [];
      for (const code of mentionedSkills(pool, general.skills)) reached.add(code);
      if (dataText) for (const code of mentionedSkills([dataText], general.skills)) reached.add(code);
      for (const x of ids) seenIds.add(x);
      run.steps.push({
        tag,
        turn: st?.turn ?? null,
        turnPhase: st?.turn_phase ?? (typeof st?.phase === "string" ? st.phase.toLowerCase() : null),
        legalCount: ids.length,
        pending,
        matchedIds: pool.filter((x) => mentionedSkills([x], general.skills).size > 0).concat(dataHits).slice(0, 10),
      });
      return { ids, pending, st };
    };

    // 第 1 步：出牌阶段（场景注入后的当前视角）。
    let { ids, pending, st } = collect(cur, "play");

    // 1a) 执行首个本将主动技（含转化技），作为「执行不报错」的证据。
    const firstSkill = ids.find((x) => /^skill:/.test(x) && mentionedSkills([x], general.skills).size > 0);
    let lastRaw = st;
    let liveIds = ids;
    if (firstSkill) {
      const acted = must(
        await api(`/internal/game/preview/${id}/action`, {
          method: "POST",
          body: { player_id: targetId, action: firstSkill },
        }),
        `action(${general.name} ${firstSkill})`,
      );
      run.chosen = firstSkill;
      run.message = acted.step?.message ?? null;
      run.events = (acted.step?.events ?? []).slice(0, 8).map((e) => ({
        type: e.event_type,
        payload: JSON.stringify(e.payload ?? {}).slice(0, 300),
      }));
      lastRaw = acted.view?.raw?.data ?? lastRaw;
      liveIds = (acted.view?.actions ?? []).map((a) => a.id);
      collect(acted.view, `after:${firstSkill}`);
    }

    // 1b) 结束出牌阶段 → 触发结束阶段/下一回合（顺序步进）。
    if (liveIds.includes("end_play")) {
      const ended = await api(`/internal/game/preview/${id}/action`, {
        method: "POST",
        body: { player_id: targetId, action: "end_play" },
      });
      if (ended.status < 300) {
        lastRaw = ended.body?.view?.raw?.data ?? lastRaw;
        collect(ended.body?.view, "end_play");
      } else {
        run.driveErrors.push(
          `end_play 在合法集中但被拒：HTTP ${ended.status} ${JSON.stringify(ended.body).slice(0, 200)}`,
        );
      }
    }

    // 2) 顺序步进：每步取本将合法动作（含 pending 选项）→ 必要时 AI 推一步。
    for (let i = 0; i < MAX_STEPS; i += 1) {
      const view = await api(
        `/internal/game/preview/${id}?raw=1&viewer=${encodeURIComponent(targetId)}&actor=${encodeURIComponent(targetId)}`,
      );
      if (view.status >= 400) {
        run.driveErrors.push(`view HTTP ${view.status}`);
        break;
      }
      const sheet = collect(view.body, `walk${i + 1}`);
      if (sheet.st?.finished === true || sheet.st?.phase === "finished") break;
      // 本将的 pending：优先选与技能相关的选项；否则取“必选项”（弃牌/确认等）继续推进，
      // 仅用于提高可达面，记录在 autoPicks 里供人工核对（不作为语义证据）。
      const ownPending =
        sheet.pending && (sheet.pending.actor === targetId || sheet.pending.actor === targetSeat);
      if (ownPending) {
        const skillPick = (sheet.pending.options ?? []).find(
          (o) => mentionedSkills([o], general.skills).size > 0,
        );
        // 依次尝试候选（技能选项优先）：4xx = 脚本猜测不合法（如缔盟手牌不足）→ 换下一个；
        // 5xx = 引擎异常 → 记失败。全部 4xx 则本将走查到此为止（不影响已达成的证据）。
        const preferred =
          (sheet.pending.options ?? []).find((o) => /(:confirm|:pass|^pass$|^skip$|^discard:|^confirm)/.test(o)) ??
          (sheet.pending.options ?? [])[0];
        const order = [
          ...(skillPick ? [skillPick] : []),
          ...(preferred && preferred !== skillPick ? [preferred] : []),
          ...(sheet.pending.options ?? []).filter((o) => o !== skillPick && o !== preferred),
        ];
        let actedOk = false;
        for (const cand of order) {
          const acted = await api(`/internal/game/preview/${id}/action`, {
            method: "POST",
            body: { player_id: targetId, action: cand },
          });
          if (acted.status < 300) {
            if (!skillPick) run.autoPicks.push(cand);
            lastRaw = acted.body?.view?.raw?.data ?? lastRaw;
            collect(acted.body?.view, `pending:${cand}`);
            actedOk = true;
            break;
          }
          if (acted.status >= 500) {
            run.driveErrors.push(`pending action ${cand} HTTP ${acted.status}`);
            break;
          }
          run.autoPicks.push(`${cand} HTTP ${acted.status}`);
        }
        if (!actedOk) break;
        continue;
      }
      const driven = await api(`/internal/game/preview/${id}/drive`, {
        method: "POST",
        body: { max_steps: 1, viewer: targetId },
      });
      if (driven.status >= 300) {
        run.driveErrors.push(`drive HTTP ${driven.status}`);
        break;
      }
      if (driven.body?.stop && driven.body.stop !== "max_steps") {
        run.stepStop = driven.body.stop;
        if (driven.body.errors?.length) run.driveErrors.push(...driven.body.errors.slice(0, 3));
        break;
      }
    }

    run.reached = [...reached];
    run.cardsAfter = countCards(lastRaw);
    run.digests.push(canonicalDigest(lastRaw));
    await api(`/internal/game/preview/${id}`, { method: "DELETE" }).catch(() => {});
    return run;
  };

  try {
    const runs = [];
    for (let i = 0; i < REPEAT; i += 1) runs.push(await attempt());
    result.runs = runs;

    result.invariants = {
      noError: runs.every((r) => (r.driveErrors ?? []).length === 0),
      conservation: runs.every((r) => r.cardsAfter === r.cardsBefore),
      pendingContinuable: runs.every((r) => !r.stepStop || r.stepStop !== "engine_error"),
      deterministic: new Set(runs.map((r) => r.digests[0])).size === 1,
    };

    const reached = [...new Set(runs.flatMap((r) => r.reached))];
    result.reachedSkills = reached;
    result.untrackedSkills = general.skills
      .filter((s) => !reached.includes(s.code))
      .map((s) => ({
        code: s.code,
        name: s.name,
        kind: s.kind,
        reason:
          s.kind === "locked"
            ? "锁定技：自动生效，无独立动作（需人工在走查中断言效果）"
            : s.kind === "lord"
              ? "主公技：需其他同势力角色响应，L1 未构造响应方"
              : s.kind === "limited" || s.kind === "awaken"
                ? "限定/觉醒：触发条件未在 L1 构造"
                : "L1 走查未出现该技能动作/待办（时机、手牌或目标不满足）",
      }));

    if (!result.invariants.noError) {
      result.status = "失败";
      result.reason = `审计流程报错：${runs.flatMap((r) => r.driveErrors).slice(0, 3).join("; ")}`;
    } else if (!result.invariants.conservation) {
      const r = runs[0];
      result.status = "失败";
      result.reason = `牌数不守恒（${r.cardsBefore} → ${r.cardsAfter}）`;
    } else if (!result.invariants.deterministic) {
      result.status = "失败";
      result.reason = "同 seed 两次 digest 不一致";
    } else if (reached.length > 0) {
      result.status = "触达";
      result.reason = `机械证据：${reached.join(",")}`;
    } else {
      result.status = "未触达";
      result.reason = `L1 未触达任何技能（${general.skills.map((s) => s.code).join(",")}）`;
    }
  } catch (e) {
    const msg = String(e?.message ?? e);
    result.status = msg.includes("失败（HTTP") ? "失败" : "阻塞";
    result.reason = msg.slice(0, 600);
    if (result.status === "阻塞") infraFailed = true;
  }
  return result;
}

/** 矩阵骨架（人工列留空；脚本不覆写已存在的矩阵，除非 --force）。 */
function renderMatrix(catalog, batches) {
  const lines = [];
  lines.push(`# 三国杀 · 全武将验证矩阵（${catalog.audited.length} 将 / 人工验收台账）`);
  lines.push("");
  lines.push(`> 口径：有界版同名的原版不进批次（用户 2026-11-05 裁定，方案 A）；${catalog.audited.length} 位 = 默认选将池（${catalog.generals.length} − ${catalog.excluded.length}）。`);
  lines.push("> 批次表单一事实源：`apps/scripts/lib/sgs-batches.mjs`（B0–B20）。");
  lines.push("> 机器证据：`docs/games/sanguosha/general-audit-evidence.md`（生成，勿手改）+ `data/audit/<batch>.json`。");
  lines.push("> **人工列（原文/歧义、人工结果、确认人、备注）只能由人工填写**；agent 不得自行标「人工通过」。");
  lines.push("");
  lines.push("## 1. 状态枚举");
  lines.push("");
  lines.push("- `未验证`：尚无机器证据也无人工走查；");
  lines.push("- `自动通过`：机器证据仅证明可达/不报错/守恒/pending 可续/可复现（**不等于语义正确**）；");
  lines.push("- `自动失败`：机器证据出现失败（需复现并登记）；");
  lines.push("- `待裁定`：技能文本存在歧义，等待用户裁定；");
  lines.push("- `人工通过` / `人工失败`：工作台逐项走查后的**唯一验收依据**（确认人签字）；");
  lines.push("- `阻塞`：环境/前置缺陷导致无法验证（注明原因）。");
  lines.push("");
  lines.push("## 2. 人工走查记录字段（逐项）");
  lines.push("");
  lines.push("```");
  lines.push("武将 / 技能 code / 原文与来源 / 歧义点或裁定 / 期望行为（人工确认）");
  lines.push("场景(batch+seed/模板) / 实际操作 / 期望结果 / 实际结果 / 结论(通过/失败/待裁定) / 确认人+日期");
  lines.push("```");
  lines.push("");
  lines.push("## 3. 矩阵");
  lines.push("");
  lines.push(
    "| 批次 | 包 | 武将 | code | 技能（类型） | 定向用例 | 原文/歧义 | 机器证据 | 人工结果 | 确认人 | 复现 | 备注 |",
  );
  lines.push("|---|---|---|---|---|---|---|---|---|---|---|---|");
  for (const b of batches) {
    for (const g of b.generals) {
      const skills = g.skills.map((s) => `${s.name}(\`${s.code}\`/${s.kind})`).join("、");
      lines.push(
        `| ${b.id} | ${g.pack} | ${g.name} | \`${g.code}\` | ${skills} | ≥3（已过门禁） | 待确认 | 见 evidence | 未验证 | - | \`--batch ${b.id} --seed ${SEED}\` | |`,
      );
    }
  }
  lines.push("");
  lines.push(`## 4. 未纳入批次的武将（${catalog.excluded.length} 位，有界版同名）`);
  lines.push("");
  lines.push("按口径 A 排除，验证由界版覆盖同名技能（如 `界刘备` 覆盖 `刘备`）：");
  lines.push("");
  const byPack = new Map();
  for (const g of catalog.excluded) {
    if (!byPack.has(g.pack)) byPack.set(g.pack, []);
    byPack.get(g.pack).push(`${g.name}→${g.excludedBy}`);
  }
  for (const [pack, list] of byPack) lines.push(`- **${pack}**（${list.length}）：${list.join("、")}`);
  lines.push("");
  return `${lines.join("\n")}\n`;
}

/** 矩阵 ↔ 名录/批次表一致性校验（防漏项/错字/批次漂移）。 */
async function checkMatrix(catalog, batches) {
  const problems = [];
  let text = "";
  try {
    text = await fs.readFile(MATRIX_FILE, "utf8");
  } catch {
    return [`矩阵不存在：${path.relative(projectRoot, MATRIX_FILE)}（跑 --matrix 生成骨架）`];
  }
  const rows = [];
  for (const line of text.split("\n")) {
    const m = /^\|\s*(B\d+)\s*\|\s*([a-z_]+)\s*\|\s*([^|]+?)\s*\|\s*`(sgs_hero_\d+)`\s*\|([^|]*)\|/.exec(
      line,
    );
    if (m) rows.push({ batch: m[1], pack: m[2], name: m[3].trim(), code: m[4], skillsCell: m[5] });
  }
  if (rows.length !== catalog.audited.length) {
    problems.push(`矩阵行数 ${rows.length} ≠ 审计范围 ${catalog.audited.length}`);
  }
  const seen = new Set();
  const byCode = new Map(catalog.audited.map((g) => [g.code, g]));
  const batchOf = new Map();
  for (const b of batches) for (const g of b.generals) batchOf.set(g.code, b.id);
  for (const row of rows) {
    if (seen.has(row.code)) problems.push(`武将重复：${row.code}`);
    seen.add(row.code);
    const g = byCode.get(row.code);
    if (!g) {
      problems.push(`矩阵含非审计范围武将：${row.name} ${row.code}`);
      continue;
    }
    if (g.name !== row.name) problems.push(`${row.code} 姓名不一致：矩阵 ${row.name} ≠ 名录 ${g.name}`);
    if (g.pack !== row.pack) problems.push(`${row.code} 包不一致：矩阵 ${row.pack} ≠ 名录 ${g.pack}`);
    if (batchOf.get(row.code) !== row.batch) {
      problems.push(`${row.code} 批次不一致：矩阵 ${row.batch} ≠ 批次表 ${batchOf.get(row.code)}`);
    }
    for (const s of g.skills) {
      if (!row.skillsCell.includes(`\`${s.code}\``)) {
        problems.push(`${row.code} 技能缺项：${s.code}`);
      }
    }
  }
  for (const g of catalog.audited) if (!seen.has(g.code)) problems.push(`矩阵漏项：${g.name} ${g.code}`);
  return problems;
}

/** 触达率聚合：按批次 / 包 / 技能类型统计「技能被机器触达」比例。 */
function reachReport(report, catalog, batches) {
  const kindOf = new Map();
  for (const g of catalog.audited) for (const s of g.skills) kindOf.set(`${g.code}::${s.code}`, s.kind);
  const add = (map, key, total, reached) => {
    const cur = map.get(key) ?? { key, total: 0, reached: 0 };
    cur.total += total;
    cur.reached += reached;
    map.set(key, cur);
  };
  const byBatch = new Map();
  const byPack = new Map();
  const byKind = new Map();
  for (const row of report.generals) {
    const meta = catalog.byCode.get(row.code);
    if (!meta) continue;
    const reached = new Set(row.reachedSkills ?? []);
    const total = row.skills.length;
    const hit = row.skills.filter((s) => reached.has(s.code)).length;
    const batchId = batches.find((b) => b.generals.some((g) => g.code === row.code))?.id ?? "-";
    add(byBatch, batchId, total, hit);
    add(byPack, meta.pack, total, hit);
    for (const s of row.skills) {
      add(byKind, kindOf.get(`${row.code}::${s.code}`) ?? s.kind ?? "unknown", 1, reached.has(s.code) ? 1 : 0);
    }
  }
  const toRows = (map) =>
    [...map.values()]
      .map((v) => ({ ...v, rate: v.total ? Math.round((v.reached / v.total) * 100) : 0 }))
      .sort((a, b) => a.key.localeCompare(b.key));
  return { byBatch: toRows(byBatch), byPack: toRows(byPack), byKind: toRows(byKind) };
}

/** 审计结果 diff：对比上一份 JSON（状态翻转 / 触达增减）。 */
function diffReport(prev, cur) {
  const prevMap = new Map((prev?.generals ?? []).map((g) => [g.code, g]));
  const changes = [];
  for (const g of cur.generals) {
    const p = prevMap.get(g.code);
    if (!p) {
      changes.push({ code: g.code, name: g.name, type: "新增", from: null, to: g.status });
      continue;
    }
    if (p.status !== g.status) {
      changes.push({ code: g.code, name: g.name, type: "状态", from: p.status, to: g.status });
    }
    const before = new Set(p.reachedSkills ?? []);
    const after = new Set(g.reachedSkills ?? []);
    const gained = [...after].filter((c) => !before.has(c));
    const lost = [...before].filter((c) => !after.has(c));
    if (gained.length || lost.length) {
      changes.push({ code: g.code, name: g.name, type: "触达", from: lost.join(",") || "-", to: gained.join(",") || "-" });
    }
  }
  return {
    previous: prev?.generatedAt ?? null,
    changes,
    summary: {
      added: changes.filter((c) => c.type === "新增").length,
      flipped: changes.filter((c) => c.type === "状态").length,
      reachChanged: changes.filter((c) => c.type === "触达").length,
    },
  };
}

/** 写证据 md（机器汇总，勿手改）。 */
async function writeEvidence(report, file) {
  const lines = [];
  lines.push("# 全武将验证 · 机器证据（自动生成，勿手改）");
  lines.push("");
  lines.push(`> 生成：\`apps/scripts/sgs-general-audit.mjs\` · 时间：${report.generatedAt}`);
  lines.push("> 只含**机械证据**（可达/不报错/守恒/pending/可复现），不含语义判定；");
  lines.push("> 人工走查结论请写入 `docs/games/sanguosha/general-audit.md`。");
  lines.push("");
  lines.push(`- 范围：${report.scope.label}（${report.scope.generals.length} 将）`);
  lines.push(`- seed=${report.seed} · repeat=${report.repeat} · players=${report.players} · maxSteps=${report.maxSteps}`);
  lines.push(
    `- 名录：${report.catalog.generals} 将（审计 ${report.catalog.audited} / 排除 ${report.catalog.excluded}）/ ${report.catalog.skills} 技能`,
  );
  const s = report.summary;
  lines.push(
    `- 结果：触达 ${s.reached} · 未触达 ${s.untracked} · 失败 ${s.failed} · 阻塞 ${s.blocked}`,
  );
  lines.push("");
  lines.push("| 批次 | 武将 | code | 技能（类型） | 状态 | 触达 | 未触达 | 不变量 | 备注 |");
  lines.push("|---|---|---|---|---|---|---|---|---|");
  for (const g of report.generals) {
    const skills = g.skills.map((x) => `${x.name}(${x.kind})`).join("、");
    const inv = Object.entries(g.invariants ?? {})
      .map(([k, v]) => `${k}=${v ? "✓" : "✗"}`)
      .join(" ");
    const untracked = g.untrackedSkills.map((x) => x.code).join(",") || "-";
    lines.push(
      `| ${report.scope.batch ?? "-"} | ${g.name} | ${g.code} | ${skills} | ${g.status} | ${g.reachedSkills.join(",") || "-"} | ${untracked} | ${inv || "-"} | ${(g.reason ?? "").replace(/\|/g, "\\|")} |`,
    );
  }
  lines.push("");
  if (report.reach) {
    lines.push("## 触达率（机器证据，供人工走查排序）");
    lines.push("");
    lines.push("| 维度 | 技能数 | 触达 | 比例 |");
    lines.push("|---|---|---|---|");
    for (const r of report.reach.byBatch) {
      lines.push(`| 批次 ${r.key} | ${r.total} | ${r.reached} | ${r.rate}% |`);
    }
    for (const r of report.reach.byKind) {
      lines.push(`| 类型 ${r.key} | ${r.total} | ${r.reached} | ${r.rate}% |`);
    }
    lines.push("");
  }
  if (report.diff?.changes?.length) {
    lines.push("## 与上一份证据的差异");
    lines.push("");
    lines.push("| 武将 | 类型 | 之前 | 现在 |");
    lines.push("|---|---|---|---|");
    for (const c of report.diff.changes) {
      lines.push(`| ${c.name} | ${c.type} | ${c.from ?? "-"} | ${c.to ?? "-"} |`);
    }
    lines.push("");
  }
  await fs.mkdir(path.dirname(file), { recursive: true });
  await fs.writeFile(file, `${lines.join("\n")}\n`, "utf8");
}

async function main() {
  const catalog = loadCatalog();
  const batches = expandBatches(catalog);

  if (flag("--list")) {
    log(
      `名录：${catalog.stats.generals} 将（审计 ${catalog.stats.audited} / 排除 ${catalog.stats.excluded}）/ ${catalog.stats.skills} 技能`,
    );
    for (const b of batches) log(`${b.id} ${String(b.generals.length).padStart(2)} 将  ${b.source}`);
    log(
      `批次表自检通过：${batches.length} 批 / ${batches.reduce((n, b) => n + b.generals.length, 0)} 将`,
    );
    return 0;
  }

  if (flag("--matrix")) {
    const exists = existsSync(MATRIX_FILE);
    if (exists && !flag("--force")) {
      log(`矩阵已存在（人工列可能在编辑中），未覆写：${path.relative(projectRoot, MATRIX_FILE)}`);
      log("如确需重建骨架，加 --force（会先备份 .bak，人工列会丢失）");
      return 0;
    }
    if (exists) await fs.copyFile(MATRIX_FILE, `${MATRIX_FILE}.bak`);
    await fs.mkdir(path.dirname(MATRIX_FILE), { recursive: true });
    await fs.writeFile(MATRIX_FILE, renderMatrix(catalog, batches), "utf8");
    log(
      `矩阵骨架已生成：${path.relative(projectRoot, MATRIX_FILE)}（${catalog.audited.length} 行 / ${batches.length} 批）`,
    );
    return 0;
  }

  if (flag("--check-matrix")) {
    const problems = await checkMatrix(catalog, batches);
    if (problems.length) {
      for (const p of problems.slice(0, 40)) console.error(`[sgs-audit] ❌ ${p}`);
      if (problems.length > 40) console.error(`[sgs-audit] …其余 ${problems.length - 40} 条`);
      return 1;
    }
    log(`矩阵一致性通过：${catalog.audited.length} 行 / ${batches.length} 批 / 技能 code 齐全`);
    return 0;
  }

  let targets = [];
  let label = "";
  let batchId = null;
  const batchArg = opt("--batch");
  const generalArg = opt("--general");
  const packArg = opt("--pack");
  const sample = Number(opt("--sample", 0)) || 0;

  if (typeof batchArg === "string") {
    const b = findBatch(batchArg);
    if (!b) throw new Error(`未知批次：${batchArg}（可选 ${BATCHES.map((x) => x.id).join(",")}）`);
    const expanded = batches.find((x) => x.id === b.id);
    targets = expanded.generals;
    label = `${b.id} ${b.source}`;
    batchId = b.id;
  } else if (typeof generalArg === "string") {
    const g = catalog.audited.find((x) => x.code === generalArg || x.name === generalArg);
    const ex = catalog.excluded.find((x) => x.code === generalArg || x.name === generalArg);
    if (!g) {
      throw new Error(
        ex
          ? `${generalArg} 有界版同名，按口径 A 已排除（${ex.excludedBy}）`
          : `${generalArg} 不在 ${catalog.audited.length} 审计范围`,
      );
    }
    targets = [g];
    label = `单将 ${g.name}`;
  } else if (typeof packArg === "string") {
    targets = catalog.audited.filter((g) => g.pack === packArg);
    label = `包 ${packArg}`;
  } else if (flag("--all")) {
    targets = catalog.audited;
    label = `全量 ${catalog.audited.length} 将`;
  } else if (flag("--check") || sample > 0) {
    const n = sample || 8;
    const picked = [];
    for (const b of batches) if (picked.length < n) picked.push(b.generals[0]);
    for (const b of batches) if (picked.length < n && b.generals[1]) picked.push(b.generals[1]);
    targets = picked.slice(0, n);
    label = `门禁抽样 ${targets.length} 将`;
  } else {
    throw new Error("需要 --batch/--general/--pack/--all/--check/--list 之一");
  }

  // 场景台账骨架（L2）：只为缺失技能补条目，绝不改写人工 expected。
  if (flag("--with-scenes") || flag("--scenes-only")) {
    const ledger = JSON.parse(await fs.readFile(SCENARIOS_FILE, "utf8").catch(() => "{}"));
    const batchFor = (g) => batches.find((b) => b.generals.some((x) => x.code === g.code))?.id ?? "B?";
    // 按批次分组合并（不同批次的同技能各自补条目）
    let added = [];
    let kept = 0;
    let current = ledger;
    const groups = new Map();
    for (const g of targets) {
      const id = batchFor(g);
      if (!groups.has(id)) groups.set(id, []);
      groups.get(id).push(g);
    }
    for (const [id, gens] of groups) {
      const r = mergeScenarios(current, id, gens, PLAYERS);
      current = r.ledger;
      added = added.concat(r.added);
      kept += r.kept;
    }
    await fs.writeFile(SCENARIOS_FILE, `${JSON.stringify(current, null, 2)}
`, "utf8");
    log(
      `台账已更新：新增 ${added.length} 条场景骨架，保留 ${kept} 条（含人工 expected）；${path.relative(projectRoot, SCENARIOS_FILE)}`,
    );
    if (flag("--scenes-only")) return 0;
  }

  const serverExe =
    process.env.SGS_AUDIT_SERVER ??
    path.join(
      projectRoot,
      "crates/target/debug",
      process.platform === "win32" ? "server.exe" : "server",
    );
  if (!existsSync(serverExe)) {
    log(`skip：server 未构建（cargo build -p server，或用 SGS_AUDIT_SERVER 指定二进制）`);
    return 0;
  }
  if (!(await tcpOk("127.0.0.1", 6379))) {
    log(`skip：Redis 不可达（${REDIS_URL}），auth 依赖`);
    return 0;
  }

  log(
    `范围：${label}（${targets.length} 将）· seed=${SEED} · repeat=${REPEAT} · maxSteps=${MAX_STEPS}${NO_SCENE ? " · no-scene（保留开局 pending）" : ""}`,
  );
  const server = spawn(serverExe, [], {
    cwd: projectRoot,
    env: {
      ...process.env,
      SQLITE_PATH,
      SERVER_PORT: String(PORT),
      REDIS_URL,
      PROPS_ADMIN_TOKEN: TOKEN,
      DEV_WHITELIST: ACTOR,
      RUST_LOG: "warn",
      SGS_PREVIEW_TTL_SECS: "1800",
      SGS_PREVIEW_MAX_SESSIONS: "64",
    },
    stdio: ["ignore", "ignore", "pipe"],
  });
  children.push(server);
  server.stderr?.on("data", () => {});
  if (!(await waitHealth(`${SERVER_URL}/health`))) {
    cleanup();
    throw new Error("临时 server 未就绪");
  }

  const dbBefore = dbCounts();
  const report = {
    generatedAt: new Date().toISOString(),
    seed: SEED,
    repeat: REPEAT,
    players: PLAYERS,
    maxSteps: MAX_STEPS,
    scope: { label, batch: batchId, generals: targets.map((g) => g.code), noScene: NO_SCENE },
    catalog: {
      generals: catalog.stats.generals,
      audited: catalog.stats.audited,
      excluded: catalog.stats.excluded,
      skills: catalog.stats.skills,
    },
    generals: [],
    summary: {},
    dbBefore,
  };

  for (const [i, g] of targets.entries()) {
    const r = await auditGeneral(g);
    report.generals.push(r);
    log(
      `${String(i + 1).padStart(2)}/${targets.length} ${r.name.padEnd(8)} ${r.status}  ${r.reason.slice(0, 70)}`,
    );
    if (r.status === "失败") failed += 1;
  }

  report.dbAfter = dbCounts();
  report.truthSourceUntouched = Object.keys(report.dbBefore).every(
    (k) => report.dbBefore[k] === report.dbAfter[k],
  );
  report.summary = {
    reached: report.generals.filter((g) => g.status === "触达").length,
    untracked: report.generals.filter((g) => g.status === "未触达").length,
    failed: report.generals.filter((g) => g.status === "失败").length,
    blocked: report.generals.filter((g) => g.status === "阻塞").length,
    truthSourceUntouched: report.truthSourceUntouched,
  };

  cleanup();
  report.reach = reachReport(report, catalog, batches);
  const diffArg = opt("--diff");
  if (typeof diffArg === "string") {
    try {
      const prev = JSON.parse(await fs.readFile(path.resolve(projectRoot, diffArg), "utf8"));
      report.diff = diffReport(prev, report);
    } catch (e) {
      log(`warn: 读取 --diff 文件失败（${diffArg}）：${e?.message ?? e}`);
    }
  }

  const outFile =
    typeof opt("--out") === "string"
      ? path.resolve(projectRoot, opt("--out"))
      : path.join(OUT_DIR, `${batchId ?? (flag("--all") ? "all" : "sample")}.json`);
  await fs.mkdir(path.dirname(outFile), { recursive: true });
  await fs.writeFile(outFile, `${JSON.stringify(report, null, 2)}\n`, "utf8");
  log(`证据 JSON：${path.relative(projectRoot, outFile)}`);

  const evidenceFile = EVIDENCE_FILE;
  await writeEvidence(report, evidenceFile);
  log(`证据汇总：${path.relative(projectRoot, evidenceFile)}`);

  log(
    `汇总：触达 ${report.summary.reached} · 未触达 ${report.summary.untracked} · 失败 ${report.summary.failed} · 阻塞 ${report.summary.blocked} · 真相源未写 ${report.summary.truthSourceUntouched ? "✓" : "✗"}`,
  );

  if (infraFailed) {
    log("阻塞：审计链路异常（server/Redis/接口），按 skip 处理");
    return 0;
  }
  return failed > 0 ? 1 : 0;
}

try {
  const code = await main();
  cleanup();
  process.exit(code);
} catch (e) {
  cleanup();
  console.error(`[sgs-audit] ❌ ${e?.message ?? e}`);
  process.exit(1);
}
