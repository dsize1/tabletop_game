// 技能对齐单（align/<B>.json ↔ align/<B>.md）共享实现。
//
// 设计要点（用户 2026-11-06 裁定）：
// - 每技能 7 段：官方原文 / 现实现分步 / 与原文差异 / 疑点+候选 / 机制依赖 / 动画·UI 期望 / 对齐后描述；
// - 「对齐后描述」是**唯一权威**，实现只引用条目编号；
// - 状态机 draft → discussed → confirmed → implemented → verified（+ blocked）；
// - 冲突：机制词表版本高于技能确认版本 → 需复核（阻断）；
// - 本文件不含 I/O 副作用以外的逻辑，供生成器与门禁共用。

import { readFileSync, readdirSync, existsSync } from "node:fs";
import path from "node:path";
import { projectRoot } from "./sgs-catalog.mjs";
import { BATCHES } from "./sgs-batches.mjs";

export const ALIGN_DIR = path.join(projectRoot, "docs/games/sanguosha/align");
export const MECHANICS_FILE = path.join(ALIGN_DIR, "mechanics.json");
const HEROES_FILE = path.join(projectRoot, "docs/games/sanguosha/data/heroes.json");

export const SKILL_STATUS = ["draft", "discussed", "confirmed", "implemented", "verified", "blocked"];
/** 状态顺序（blocked 不参与比较）。 */
export const STATUS_ORDER = ["draft", "discussed", "confirmed", "implemented", "verified"];
export const atLeast = (status, want) => STATUS_ORDER.indexOf(status) >= STATUS_ORDER.indexOf(want);

export function loadMechanics() {
  return JSON.parse(readFileSync(MECHANICS_FILE, "utf8"));
}

/** gid → { name, skills: [{name, description, type_guess}] }（官网原文）。 */
export function loadHeroesByGid() {
  const raw = JSON.parse(readFileSync(HEROES_FILE, "utf8"));
  const out = new Map();
  for (const key of Object.keys(raw.heroes)) {
    const h = raw.heroes[key];
    out.set(Number(h.gid), h);
  }
  return out;
}

export function specPath(batch) {
  return path.join(ALIGN_DIR, `${batch}.json`);
}

export function listSpecBatches() {
  if (!existsSync(ALIGN_DIR)) return [];
  return readdirSync(ALIGN_DIR)
    .filter((f) => /^B\d+\.json$/.test(f))
    .map((f) => f.replace(/\.json$/, ""))
    .sort((a, b) => Number(a.slice(1)) - Number(b.slice(1)));
}

export function loadSpec(batch) {
  const file = specPath(batch);
  if (!existsSync(file)) throw new Error(`对齐单不存在：${file}`);
  return JSON.parse(readFileSync(file, "utf8"));
}

/** 由 catalog + 批次表 + heroes.json 生成骨架。 */
/** heroes.json 的部分描述是重复拼接（`###转换技：` 等噪声）→ 取第一段并去重。 */
export function cleanOriginal(text) {
  if (!text) return text;
  const first = text.split("###")[0];
  return first.trim();
}

export function skeletonSpec(batchId, catalog, batches) {
  const batch = batches.find((b) => b.id === batchId);
  if (!batch) throw new Error(`未知批次：${batchId}`);
  const heroes = loadHeroesByGid();
  return {
    batch: batch.id,
    source: batch.source,
    status: "draft",
    mechanics_version: loadMechanics().version,
    confirmed_by: null,
    confirmed_at: null,
    note: "机器文件（单一事实源）。人读请看同目录 <B>.md；改这里后运行 --sync 重生成 md。",
    generals: batch.generals.map((g) => {
      const h = heroes.get(g.gid);
      return {
        gid: g.gid,
        code: g.code,
        name: g.name,
        skills: g.skills
          .filter((s) => s.implemented)
          .map((s) => {
            const official = (h?.skills ?? []).find((x) => x.name === s.name);
            return {
              code: s.code,
              name: s.name,
              kind: s.kind,
              status: "draft",
              original: cleanOriginal(official?.description) ?? null,
              original_source: official ? `heroes.json#gid=${g.gid}` : null,
              mechanics_version: loadMechanics().version,
              current: [],
              confidence: "inferred",
              diff: [],
              questions: [],
              mechanics: [],
              terms: {},
              ui: { prompt: null, overlay: null, fx: null, reveal: null, qq: null },
              agreed: null,
              assertions: { positive: [], negative: [] },
              impl: null,
              tests: [],
            };
          }),
      };
    }),
    spotcheck: [],
  };
}

// ---------------------------------------------------------------------------
// 渲染（json → md）
// ---------------------------------------------------------------------------

const uiCell = (v) => (v == null || v === "" ? "（待填）" : String(v));

/** 原文里的关键限定语：agreed 必须复述（防「凭记忆压缩」导致语义偏移）。 */
export const KEY_PHRASES = [
  "指定目标后",
  "可以",
  "然后",
  "成为目标",
  "回合外",
  "结束阶段",
  "准备阶段",
  "出牌阶段",
  "摸牌阶段",
  "弃牌阶段",
  "每回合",
  "限一次",
  "锁定技",
  "主公技",
  "转换技",
  "移出游戏",
  "装备区",
  "判定区",
  "手牌上限",
];
const esc = (s) => String(s ?? "").replace(/\|/g, "\\|");

export function renderMarkdown(spec, catalog, mechanics) {
  const L = [];
  const allSkills = [];
  for (const g of spec.generals) for (const s of g.skills) allSkills.push({ g, s });

  L.push(`# 对齐单 ${spec.batch} · ${spec.source}（${spec.generals.length} 将 / ${allSkills.length} 技能）`);
  L.push("");
  L.push("> **本文件由 `npm run sgs:align -- --batch " + spec.batch + " --sync` 生成，勿手改**；改 `" + spec.batch + ".json`。");
  L.push(`> 批次状态：\`${spec.status}\`｜机制词表版本：${spec.mechanics_version}（当前 ${mechanics.version}）｜确认人：${spec.confirmed_by ?? "-"}｜确认时间：${spec.confirmed_at ?? "-"}`);
  L.push("> 流程与字段说明见 `README.md`；门禁 `npm run check:sgs-align`。");
  L.push("");

  const qs = allSkills.flatMap(({ g, s }) => s.questions.map((q) => ({ g, s, q })));
  L.push(`## 0. 本批疑点汇总（${qs.length} 条，按此逐条回答即可）`);
  L.push("");
  if (qs.length === 0) {
    L.push("本批暂无疑点：确认「同意现实现」即可把各技能状态改为 `confirmed`。");
  } else {
    L.push("| # | 武将 | 技能 | 疑点 | 候选 | agent 倾向 | 影响面 | 你的裁定 |");
    L.push("|---|---|---|---|---|---|---|---|");
    qs.forEach(({ g, s, q }, i) => {
      const opts = Object.entries(q.options ?? {})
        .map(([k, v]) => `${k}. ${esc(v)}`)
        .join("<br>");
      L.push(
        `| ${i + 1} | ${g.name} | ${s.name} | ${esc(q.ask)} | ${opts} | ${q.lean ?? "-"} | ${esc(q.impact ?? "-")} | ${q.answer ? esc(q.answer) : "（待答）"} |`,
      );
    });
  }
  L.push("");

  const byStatus = {};
  for (const { s } of allSkills) byStatus[s.status] = (byStatus[s.status] ?? 0) + 1;
  L.push(
    "## 0.1 状态分布：" +
      Object.entries(byStatus)
        .map(([k, v]) => `${k}=${v}`)
        .join(" / "),
  );
  L.push("");

  for (const g of spec.generals) {
    L.push(`---`);
    L.push("");
    L.push(`# ${g.name} \`${g.code}\`（${g.skills.length} 技能）`);
    for (const s of g.skills) {
      L.push("");
      L.push(`## ${s.name}（\`${s.code}\` / ${s.kind}）— \`${s.status}\``);
      L.push("");
      L.push(`**【1 官方原文】** ${s.original ? esc(s.original) : "（heroes.json 未收录，需人工补录）"}`);
      if (s.original_source) L.push(`来源：\`${s.original_source}\``);
      L.push("");
      L.push("**【2 现实现（按代码陈述）】**");
      if (s.current.length === 0) L.push("- （待 agent 填写）");
      for (const step of s.current) L.push(`- ${step}`);
      L.push(`置信度：\`${s.confidence}\`（verified=已核对代码路径；inferred=推断，未找到唯一路径）`);
      L.push("");
      L.push("**【3 与原文差异】**");
      if (s.diff.length === 0) L.push("- 无（或待补）");
      for (const d of s.diff) L.push(`- ${d}`);
      L.push("");
      L.push("**【4 疑点】**");
      if (s.questions.length === 0) L.push("- 无");
      for (const q of s.questions) {
        const opts = Object.entries(q.options ?? {})
          .map(([k, v]) => `${k}. ${v}`)
          .join(" / ");
        L.push(`- \`${q.id}\` ${q.ask}`);
        L.push(`  - 候选：${opts}`);
        L.push(`  - 倾向：**${q.lean ?? "-"}**${q.why ? `（${q.why}）` : ""}`);
        L.push(`  - 影响：${q.impact ?? "-"}`);
        L.push(`  - 裁定：${q.answer ? `**${q.answer}**` : "（待你回答）"}`);
      }
      L.push("");
      L.push(
        `**【5 机制依赖】** ${s.mechanics.length ? s.mechanics.map((m) => `\`${m}\``).join("、") : "（无）"}`,
      );
      L.push("");
      L.push("**【6 动画 / UI 期望】**");
      L.push(
        `- 提示文案：${uiCell(s.ui.prompt)}｜浮层交互：${uiCell(s.ui.overlay)}｜动效 key：${uiCell(s.ui.fx)}｜逐张亮牌：${uiCell(s.ui.reveal)}｜群侧播报：${uiCell(s.ui.qq)}`,
      );
      L.push("");
      L.push("**【7 对齐后描述（权威）】**");
      L.push(s.agreed ? s.agreed : "（待你确认后填写）");
      L.push("");
      L.push("**断言**");
      L.push(`- 正例：${s.assertions.positive.length ? s.assertions.positive.join("；") : "（待填）"}`);
      L.push(`- 反例：${s.assertions.negative.length ? s.assertions.negative.join("；") : "（待填）"}`);
      L.push(`- 实现锚点：${s.impl ?? "（待填）"}｜用例：${s.tests.length ? s.tests.join("、") : "（待填）"}`);
    }
    L.push("");
  }

  L.push("---");
  L.push("");
  L.push("## 本批抽检（验收用，抽 2–3 个高风险技能真机走查）");
  L.push("");
  L.push("| 技能 | 场景(seed/模板) | 期望 | 实际 | 结论 | 确认人+日期 |");
  L.push("|---|---|---|---|---|---|");
  if (spec.spotcheck.length === 0) {
    L.push("| （待抽检） | | | | | |");
  } else {
    for (const r of spec.spotcheck) {
      L.push(
        `| ${r.skill} | ${r.scene ?? "-"} | ${esc(r.expect ?? "-")} | ${esc(r.actual ?? "-")} | ${r.result ?? "-"} | ${r.by ?? "-"} |`,
      );
    }
  }
  L.push("");
  return L.join("\n");
}

// ---------------------------------------------------------------------------
// 校验
// ---------------------------------------------------------------------------

/**
 * 单批校验。返回 { errors, warnings, conflicts }。
 * `strict` = 门禁模式（confirmed 及以上要求全部字段齐备）。
 */
export function validateSpec(spec, catalog, mechanics, { strict = true } = {}) {
  const errors = [];
  const warnings = [];
  const conflicts = [];
  const batchId = spec.batch;
  const batch = BATCHES.find((b) => b.id === batchId);
  if (!batch) return { errors: [`未知批次 ${batchId}`], warnings, conflicts };
  // catalog.byCode 是「原始名录」（不含 skills）；校验技能需用 audited 条目。
  const auditByCode = new Map(catalog.audited.map((g) => [g.code, g]));

  // 批次构成 ↔ catalog
  const wantNames = batch.names.join(",");
  const gotNames = spec.generals.map((g) => g.name).join(",");
  if (wantNames !== gotNames) {
    errors.push(`批次成员与 sgs-batches.mjs 不一致：期望 [${wantNames}]，实际 [${gotNames}]`);
  }
  const mechIds = new Set(mechanics.mechanics.map((m) => m.id));

  for (const g of spec.generals) {
    const cg = auditByCode.get(g.code);
    if (!cg) {
      errors.push(`${g.name}: code ${g.code} 不在 data.rs`);
      continue;
    }
    if (cg.name !== g.name) errors.push(`${g.code}: 名字 ${g.name} ≠ data.rs ${cg.name}`);
    const impl = cg.skills.filter((s) => s.implemented).map((s) => s.code);
    const specCodes = g.skills.map((s) => s.code);
    for (const code of impl) {
      if (!specCodes.includes(code)) errors.push(`${g.name}: 漏技能 ${code}`);
    }
    for (const code of specCodes) {
      if (!impl.includes(code)) errors.push(`${g.name}: 多出未实现技能 ${code}`);
    }

    for (const s of g.skills) {
      const tag = `${g.name}·${s.name}(${s.code})`;
      if (!SKILL_STATUS.includes(s.status)) errors.push(`${tag}: 非法状态 ${s.status}`);
      if (!s.original) errors.push(`${tag}: 缺官方原文（heroes.json 未收录需人工补录）`);
      for (const m of s.mechanics) {
        if (!mechIds.has(m)) errors.push(`${tag}: 机制引用不存在 \`${m}\``);
      }
      if (atLeast(s.status, "discussed")) {
        if (s.current.length < 3) errors.push(`${tag}: 现实现陈述至少 3 条（当前 ${s.current.length}）`);
        if (s.mechanics.length === 0) warnings.push(`${tag}: 未声明机制依赖（建议至少 1 条）`);
      }
      if (atLeast(s.status, "confirmed")) {
        if (!s.agreed || s.agreed.length < 20) errors.push(`${tag}: 缺「对齐后描述」（权威文本，≥20 字）`);
        const unanswered = s.questions.filter((q) => !q.answer);
        if (unanswered.length) {
          errors.push(`${tag}: 疑点未答 ${unanswered.map((q) => q.id).join(",")}`);
        }
        if (s.mechanics_version < mechanics.version) {
          conflicts.push(
            `${tag}: 机制词表已从 v${s.mechanics_version} 升到 v${mechanics.version}，引用 [${s.mechanics.join(",")}]，需复核对齐后描述`,
          );
        }
      }
      if (atLeast(s.status, "implemented")) {
        if (!s.impl) errors.push(`${tag}: 缺实现锚点 impl（如 sanguosha2::skills::liangyin）`);
        if (s.tests.length === 0) errors.push(`${tag}: 缺回归用例 tests`);
        if (s.assertions.positive.length === 0) errors.push(`${tag}: 缺正例断言`);
        if (s.assertions.negative.length === 0) errors.push(`${tag}: 缺反例断言（良姻教训：误触发必须有反例）`);
      }
      // 「原文限定语复述」检查：agreed 不得省略原文的关键限定词（2026-11-06 奇制「指定目标后」事故）。
      if (s.agreed) {
        for (const phrase of KEY_PHRASES) {
          if (s.original?.includes(phrase) && !s.agreed.includes(phrase)) {
            warnings.push(`${tag}: 对齐后描述未复述原文限定语「${phrase}」`);
          }
        }
      }
      if (s.ui && Object.values(s.ui).every((v) => v == null || v === "")) {
        if (atLeast(s.status, "confirmed") && strict) {
          warnings.push(`${tag}: UI 段全空（确认后应填写或写「本期不做」）`);
        }
      }
    }
  }

  if (atLeast(spec.status, "confirmed") && !spec.confirmed_by) {
    errors.push(`批次 ${batchId}: 状态 ${spec.status} 但缺 confirmed_by`);
  }
  if (conflicts.length && spec.status !== "blocked") {
    errors.push(`批次 ${batchId}: 存在机制冲突但批次状态未置 blocked`);
  }
  return { errors, warnings, conflicts };
}

/** 跨批术语冲突检测：同一术语在不同技能里定义不一致。 */
export function detectTermConflicts(specs) {
  const seen = new Map();
  const conflicts = [];
  for (const spec of specs) {
    for (const g of spec.generals) {
      for (const s of g.skills) {
        for (const [term, def] of Object.entries(s.terms ?? {})) {
          const prev = seen.get(term);
          if (!prev) {
            seen.set(term, { def, tag: `${spec.batch}/${g.name}·${s.name}` });
          } else if (prev.def !== def) {
            conflicts.push(
              `术语「${term}」定义冲突：${prev.tag} = 「${prev.def}」 vs ${spec.batch}/${g.name}·${s.name} = 「${def}」`,
            );
          }
        }
      }
    }
  }
  return conflicts;
}
