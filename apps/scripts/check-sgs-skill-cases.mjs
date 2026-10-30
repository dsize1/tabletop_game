#!/usr/bin/env node
// P4.1-D 门禁：标准 25 将「每武将 ≥3 条规则用例」。
//
// 统计口径：扫描 `crates/engines/sanguosha/src/tests/` 下所有 `#[test]` 函数体，
// 若测试体出现该武将的 `sgs_hero_<gid>` 或该武将任一技能的 code（带引号），
// 即认为该用例覆盖该武将。跨技能交互用例同时计入涉及的多个武将。
//
// 用法：node apps/scripts/check-sgs-skill-cases.mjs [--min 3] [--verbose]

import { readdir, readFile } from "node:fs/promises";
import { resolve } from "node:path";

const TESTS_DIR = resolve("crates", "engines", "sanguosha", "src", "tests");

/// gid → 技能 code（与 `crates/engines/sanguosha/src/skills.rs` 保持一致）。
const GENERAL_SKILLS = {
  1: ["rende", "jijiang"],
  2: ["wusheng"],
  3: ["paoxiao"],
  4: ["guanxing", "kongcheng"],
  5: ["longdan"],
  6: ["tieqi", "mashu"],
  7: ["jizhi", "qicai"],
  8: ["zhiheng", "jiuyuan"],
  9: ["qixi"],
  10: ["keji"],
  11: ["kurou"],
  12: ["yingzi", "fanjian"],
  13: ["guose", "liuli"],
  14: ["qianxun", "lianying"],
  15: ["jianxiong", "hujia"],
  16: ["fankui", "guicai"],
  17: ["ganglie"],
  18: ["tuxi"],
  19: ["luoyi"],
  20: ["tiandu", "yiji"],
  21: ["luoshen", "qingguo"],
  22: ["jijiu", "qingnang"],
  23: ["wushuang"],
  24: ["biyue", "lijian"],
  25: ["xiaoji", "jieyin"],
};

const GENERAL_NAMES = {
  1: "刘备", 2: "关羽", 3: "张飞", 4: "诸葛亮", 5: "赵云",
  6: "马超", 7: "黄月英", 8: "孙权", 9: "甘宁", 10: "吕蒙",
  11: "黄盖", 12: "周瑜", 13: "大乔", 14: "陆逊", 15: "曹操",
  16: "司马懿", 17: "夏侯惇", 18: "张辽", 19: "许褚", 20: "郭嘉",
  21: "甄姬", 22: "华佗", 23: "吕布", 24: "貂蝉", 25: "孙尚香",
};

function parseArgs(argv) {
  const opts = { min: 3, verbose: false };
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === "--min") opts.min = Number(argv[++i]);
    else if (argv[i] === "--verbose" || argv[i] === "-v") opts.verbose = true;
    else if (argv[i] === "--help" || argv[i] === "-h") {
      console.log("用法：node apps/scripts/check-sgs-skill-cases.mjs [--min 3] [--verbose]");
      process.exit(0);
    } else {
      console.error(`未知参数：${argv[i]}`);
      process.exit(1);
    }
  }
  if (!Number.isInteger(opts.min) || opts.min < 1) {
    console.error("--min 必须为正整数");
    process.exit(1);
  }
  return opts;
}

const TEST_RE = /#\[test\][\s\S]*?\n\s*fn\s+([A-Za-z0-9_]+)\s*\(/g;

async function collectTests() {
  const files = (await readdir(TESTS_DIR)).filter((f) => f.endsWith(".rs"));
  const tests = [];
  for (const file of files) {
    const text = await readFile(resolve(TESTS_DIR, file), "utf8");
    const marks = [...text.matchAll(TEST_RE)].map((m) => ({
      name: m[1],
      start: m.index,
    }));
    for (let i = 0; i < marks.length; i++) {
      const start = marks[i].start;
      const end = i + 1 < marks.length ? marks[i + 1].start : text.length;
      tests.push({ file, name: marks[i].name, body: text.slice(start, end) });
    }
  }
  return tests;
}

function covers(body, gid) {
  if (body.includes(`sgs_hero_${gid}`)) return true;
  return (GENERAL_SKILLS[gid] ?? []).some((code) => body.includes(`"${code}"`));
}

const opts = parseArgs(process.argv.slice(2));
const tests = await collectTests();
if (!tests.length) {
  console.error(`[check] 未找到任何 #[test]：${TESTS_DIR}`);
  process.exit(1);
}

const counts = {};
const names = {};
for (const gid of Object.keys(GENERAL_SKILLS).map(Number)) {
  counts[gid] = 0;
  names[gid] = [];
}
for (const t of tests) {
  for (const gid of Object.keys(GENERAL_SKILLS).map(Number)) {
    if (covers(t.body, gid)) {
      counts[gid] += 1;
      names[gid].push(`${t.file}::${t.name}`);
    }
  }
}

const missing = [];
for (const gid of Object.keys(GENERAL_SKILLS).map(Number)) {
  if (opts.verbose || counts[gid] < opts.min) {
    console.log(`  ${counts[gid]}\t${GENERAL_NAMES[gid]} (sgs_hero_${gid})`);
  }
  if (counts[gid] < opts.min) missing.push(gid);
}

if (missing.length) {
  console.error(
    `[check] 失败：${missing.length} 位武将用例不足 ${opts.min} 条：` +
      missing.map((gid) => `${GENERAL_NAMES[gid]}(sgs_hero_${gid})=${counts[gid]}`).join("、"),
  );
  for (const gid of missing) {
    console.error(`  ${GENERAL_NAMES[gid]} 现有用例：${names[gid].join(", ") || "（无）"}`);
  }
  process.exit(1);
}

console.log(
  `[check] OK：${tests.length} 条测试 / 25 位武将均 ≥${opts.min} 条（最低 ${Math.min(
    ...Object.values(counts),
  )}）`,
);
