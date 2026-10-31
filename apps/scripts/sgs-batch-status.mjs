#!/usr/bin/env node
// 三国杀批次进度 / 一致性检查（P4.4.2 harness）。
//
// 输出：
// - 各武将包「总武将数 / 已实现 / 剩余」；
// - `development-plan.md` 批次表每批完成度与缺失武将；
// - `p67` 中「一将成名已实现 N 位」硬编码断言与代码实际值是否一致。
//
// 用法：
//   node apps/scripts/sgs-batch-status.mjs           # 只报告
//   node apps/scripts/sgs-batch-status.mjs --check   # 不一致时 exit 1（供聚合门禁调用）
//   node apps/scripts/sgs-batch-status.mjs --json    # 机器可读输出

import {
  PACK_LABEL,
  batchReport,
  loadGenerals,
  loadP67Count,
} from "./sgs-data-lib.mjs";

const args = new Set(process.argv.slice(2));
const check = args.has("--check");
const jsonOut = args.has("--json");

const { ready, report: batchReportRows } = batchReport();
const generals = loadGenerals();
const p67 = loadP67Count();

const byPack = new Map();
for (const g of generals.values()) {
  const p = byPack.get(g.pack) ?? { total: 0, ready: 0, remaining: [] };
  p.total += 1;
  if (ready.has(g.gid)) p.ready += 1;
  else p.remaining.push(`${g.gid} ${g.name}`);
  byPack.set(g.pack, p);
}

const batchReportOut = batchReportRows.map((b) => ({
  batch: b.batch,
  total: b.total,
  done: b.doneCount,
  missing: b.missing,
  markedDone: b.done,
}));
const nextBatch = batchReportOut.find((b) => b.done < b.total) ?? null;

const yijiang = byPack.get("yijiang") ?? { ready: 0, total: 0, remaining: [] };
const problems = [];
if (p67 !== null && p67 !== yijiang.ready) {
  problems.push(
    `p67「一将成名已实现 ${p67} 位」与代码实际 ${yijiang.ready} 位不一致（tests/p67.rs）`,
  );
}
for (const b of batchReportOut) {
  const allDone = b.done === b.total;
  if (b.markedDone && !allDone) {
    problems.push(
      `批次 ${b.batch} 标了 ✅ 但仍有未实现武将：${b.missing.map((m) => `${m.gid} ${m.name}`).join("、")}`,
    );
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
        batches: batchReportOut,
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
  for (const b of batchReportOut) {
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
