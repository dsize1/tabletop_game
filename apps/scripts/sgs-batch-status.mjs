#!/usr/bin/env node
// 三国杀批次进度 / 一致性检查（P4.4.2 harness）。
//
// 输出：
// - 各武将包「总武将数 / 已实现 / 剩余」；
// - `development-plan.md` 批次表（Y1–Y7）每批完成度与缺失武将；
// - `p67` 中「一将成名已实现 N 位」硬编码断言与代码实际值是否一致。
//
// 用法：
//   node apps/scripts/sgs-batch-status.mjs           # 只报告
//   node apps/scripts/sgs-batch-status.mjs --check   # 不一致时 exit 1（供聚合门禁调用）
//   node apps/scripts/sgs-batch-status.mjs --json    # 机器可读输出

import { readFileSync } from "node:fs";

const SKILLS_RS = "crates/engines/sanguosha/src/skills.rs";
const DATA_RS = "crates/engines/sanguosha/src/data.rs";
const PLAN_MD = "docs/games/sanguosha/development-plan.md";
const P67_RS = "crates/engines/sanguosha/src/tests/p67.rs";

const PACK_LABEL = {
  standard: "标准",
  wind_fire_forest_mountain: "风林火山",
  yijiang: "一将成名",
  jiexian: "界限突破",
};

const args = new Set(process.argv.slice(2));
const check = args.has("--check");
const jsonOut = args.has("--json");

function read(path) {
  return readFileSync(path, "utf8");
}

/** skills.rs：已实现武将 code 集合（技能非空且全部 implemented:true）。 */
function loadReadyGenerals() {
  const text = read(SKILLS_RS);
  const consts = new Map();
  const defRe = /const\s+([A-Z0-9_]+):\s*&\[SkillDef\]\s*=\s*&\[([\s\S]*?)\n\s*\}?\];/g;
  for (const m of text.matchAll(defRe)) {
    const entries = [];
    for (const e of m[2].split("SkillDef {").slice(1)) {
      const code = /code:\s*"([a-z0-9_]+)"/.exec(e)?.[1];
      if (!code) continue;
      entries.push({ code, implemented: /implemented:\s*true/.test(e) });
    }
    consts.set(m[1], entries);
  }
  const ready = new Map();
  for (const m of text.matchAll(/"sgs_hero_(\d+)"\s*=>\s*([A-Z0-9_]+)/g)) {
    const gid = Number(m[1]);
    const skills = consts.get(m[2]);
    if (!skills || skills.length === 0) continue;
    if (!skills.every((s) => s.implemented)) continue;
    ready.set(gid, skills.map((s) => s.code));
  }
  return ready;
}

/** data.rs：gid → { code, name, pack }。 */
function loadGenerals() {
  const text = read(DATA_RS);
  const out = new Map();
  const re =
    /code: "(sgs_hero_\d+)", gid: (\d+), name: "([^"]+)", pack: "([a-z_]+)"/g;
  for (const m of text.matchAll(re)) {
    out.set(Number(m[2]), { code: m[1], gid: Number(m[2]), name: m[3], pack: m[4] });
  }
  return out;
}

/** development-plan.md：批次表行。 */
function loadBatches() {
  const text = read(PLAN_MD);
  const batches = [];
  for (const line of text.split(/\r?\n/)) {
    const m = /^\|\s*(Y\d+)(\s*✅)?\s*\|\s*(.+?)\s*\|\s*$/.exec(line);
    if (!m) continue;
    const entries = [];
    for (const e of m[3].matchAll(/(\d{3})\s+([^、|（(]+)/g)) {
      entries.push({ gid: Number(e[1]), name: e[2].trim() });
    }
    if (entries.length === 0) continue;
    batches.push({ batch: m[1], done: Boolean(m[2]), entries });
  }
  return batches;
}

/** p67：「一将成名已实现 N 位」断言值。 */
function loadP67Count() {
  const text = read(P67_RS);
  const m = /assert_eq!\(\s*yijiang\.len\(\),\s*(\d+)/.exec(text);
  return m ? Number(m[1]) : null;
}

const ready = loadReadyGenerals();
const generals = loadGenerals();
const batches = loadBatches();
const p67 = loadP67Count();

const byPack = new Map();
for (const g of generals.values()) {
  const p = byPack.get(g.pack) ?? { total: 0, ready: 0, remaining: [] };
  p.total += 1;
  if (ready.has(g.gid)) p.ready += 1;
  else p.remaining.push(`${g.gid} ${g.name}`);
  byPack.set(g.pack, p);
}

const batchReport = batches.map((b) => {
  const done = b.entries.filter((e) => ready.has(e.gid));
  const missing = b.entries.filter((e) => !ready.has(e.gid));
  return {
    batch: b.batch,
    total: b.entries.length,
    done: done.length,
    missing,
    markedDone: b.done,
  };
});
const nextBatch = batchReport.find((b) => b.done < b.total) ?? null;

const yijiang = byPack.get("yijiang") ?? { ready: 0, total: 0, remaining: [] };
const problems = [];
if (p67 !== null && p67 !== yijiang.ready) {
  problems.push(
    `p67「一将成名已实现 ${p67} 位」与代码实际 ${yijiang.ready} 位不一致（tests/p67.rs）`,
  );
}
for (const b of batchReport) {
  const allDone = b.done === b.total;
  if (b.markedDone && !allDone) {
    problems.push(`批次 ${b.batch} 标了 ✅ 但仍有未实现武将：${b.missing.map((m) => `${m.gid} ${m.name}`).join("、")}`);
  }
  if (!b.markedDone && allDone) {
    problems.push(`批次 ${b.batch} 已全部实现但计划表未标 ✅`);
  }
}

if (jsonOut) {
  console.log(
    JSON.stringify(
      {
        packs: Object.fromEntries(byPack),
        batches: batchReport,
        nextBatch,
        p67,
        yijiangReady: yijiang.ready,
        problems,
      },
      null,
      2,
    ),
  );
} else {
  console.log("[sgs] 武将包进度：");
  for (const [key, p] of byPack) {
    console.log(
      `  ${(PACK_LABEL[key] ?? key).padEnd(5, "　")} ${String(p.ready).padStart(3)} / ${String(p.total).padStart(3)} 已实现` +
        (p.remaining.length ? `（剩余 ${p.remaining.length}）` : "（全部完成）"),
    );
  }
  console.log("[sgs] 一将成名批次：");
  for (const b of batchReport) {
    const mark = b.markedDone ? "✅" : b.done === b.total ? "✅(未标记)" : "  ";
    console.log(
      `  ${b.batch} ${mark} ${b.done}/${b.total}` +
        (b.missing.length
          ? `  缺失：${b.missing.map((m) => `${m.gid} ${m.name}`).join("、")}`
          : ""),
    );
  }
  console.log(
    `[sgs] p67 断言：一将成名 ${p67 ?? "未找到"} 位 / 代码实际 ${yijiang.ready} 位`,
  );
  if (nextBatch) {
    console.log(
      `[sgs] 下一批：${nextBatch.batch}（${nextBatch.done}/${nextBatch.total}）→ ` +
        nextBatch.missing.map((m) => `${m.gid} ${m.name}`).join("、"),
    );
  } else {
    console.log("[sgs] 一将成名批次表已全部完成 🎉");
  }
  if (problems.length) {
    console.log("[sgs] ⚠️ 一致性问题：");
    for (const p of problems) console.log(`  - ${p}`);
  } else {
    console.log("[sgs] ✅ 批次进度与计划表/p67 断言一致");
  }
}

if (check && problems.length) process.exitCode = 1;
