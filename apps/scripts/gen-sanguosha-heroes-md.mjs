#!/usr/bin/env node
// 由官网抓取数据 heroes.json 生成武将表 docs/games/sanguosha/generals.md。
//
// 用法：
//   node apps/scripts/gen-sanguosha-heroes-md.mjs
//   node apps/scripts/gen-sanguosha-heroes-md.mjs --in docs/games/sanguosha/data/heroes.json --out docs/games/sanguosha/generals.md

import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";

const DEFAULT_IN = resolve("docs", "games", "sanguosha", "data", "heroes.json");
const DEFAULT_OUT = resolve("docs", "games", "sanguosha", "generals.md");

const PACK_ORDER = [
  ["标准", "standard"],
  ["风林火山", "wind_fire_forest_mountain"],
  ["一将成名", "yijiang"],
  ["界限突破", "boundary_break"],
  ["神将", "god"],
  ["谋", "mou"],
  ["门阀士族", "clan"],
  ["魔", "demon"],
  ["国战", "guozhan"],
  ["其他", "other"],
];

const TYPE_LABEL = {
  lord: "主公技",
  normal: "一般",
  locked: "锁定技",
  limited: "限定技",
  awakening: "觉醒技",
};

function parseArgs(argv) {
  const opts = { in: DEFAULT_IN, out: DEFAULT_OUT };
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === "--in") opts.in = resolve(argv[++i]);
    else if (argv[i] === "--out") opts.out = resolve(argv[++i]);
    else if (argv[i] === "--help" || argv[i] === "-h") {
      console.log("用法：node apps/scripts/gen-sanguosha-heroes-md.mjs [--in heroes.json] [--out generals.md]");
      process.exit(0);
    } else {
      console.error(`未知参数：${argv[i]}`);
      process.exit(1);
    }
  }
  return opts;
}

function cell(s) {
  return String(s ?? "")
    .replace(/\r?\n/g, "<br>")
    .replace(/\|/g, "\\|")
    .trim();
}

function skillsCell(skills) {
  if (!skills || skills.length === 0) return "—";
  return skills
    .map((s) => {
      const label = s.type_guess ? `（${TYPE_LABEL[s.type_guess] ?? s.type_guess}）` : "";
      return `**${cell(s.name)}**${label}：${cell(s.description) || "（官网描述为空）"}`;
    })
    .join("<br>");
}

function row(h) {
  return `| ${h.gid} | ${cell(h.name)} | ${cell(h.kingdom)} | ${h.hp} | ${cell(h.quality) || "—"} | ${skillsCell(h.skills)} | ${cell(h.characteristic) || "—"} | ${cell(h.biography) || "—"} |`;
}

async function main() {
  const opts = parseArgs(process.argv.slice(2));
  const data = JSON.parse(await readFile(opts.in, "utf8"));
  const heroes = data.heroes ?? [];

  const lines = [];
  lines.push("# 三国杀武将表");
  lines.push("");
  lines.push("> 本文件由 `docs/games/sanguosha/data/heroes.json` 自动生成，**请勿手工编辑**；数据更新后重新运行生成脚本。");
  lines.push("> 数据来源：三国杀官网公开页面（`https://www.sanguosha.com/hero`），仅内部开发/初始化使用，上线前确认授权。");
  lines.push(`> 生成时间：${data.fetched_at ?? "—"}；武将 ${heroes.length} 位，技能 ${heroes.reduce((a, h) => a + (h.skills?.length ?? 0), 0)} 条。`);
  lines.push("");
  lines.push("说明：");
  lines.push("");
  lines.push("- 技能类型为**官网描述前缀推断**（主公技/锁定技/限定技/觉醒技/一般），需人工复核。");
  lines.push("- `性别` 官网未提供，未列入本表（中台补）；`initial_hp` 官网全为 0（按 = 体力处理），未单列。");
  lines.push("- `avatar`/`cover` 为官方图片 URL，不用于本项目（素材自行制作），未列入本表。");
  lines.push("- `国战` 包为半体力（1.5/2），身份局导入时排除。");
  lines.push("");
  lines.push("## 目录");
  lines.push("");
  for (const [pack] of PACK_ORDER) {
    const list = heroes.filter((h) => h.pack === pack);
    if (list.length > 0) lines.push(`- [${pack}](#${pack})（${list.length}）`);
  }
  lines.push("");

  for (const [pack, code] of PACK_ORDER) {
    const list = heroes.filter((h) => h.pack === pack).sort((a, b) => a.gid - b.gid);
    if (list.length === 0) continue;
    lines.push(`## ${pack}`);
    lines.push("");
    lines.push(`> 包 code：\`${code}\`；共 ${list.length} 位。`);
    lines.push("");
    lines.push("| gid | 名称 | 势力 | 体力 | 品质 | 技能 | 武将特点 | 生平 |");
    lines.push("|---:|---|---|---:|---|---|---|---|");
    for (const h of list) lines.push(row(h));
    lines.push("");
  }

  await writeFile(opts.out, lines.join("\n"), "utf8");
  console.log(`[gen] ${heroes.length} 位武将 → ${opts.out}`);
}

main().catch((err) => {
  console.error("[gen] 失败:", err);
  process.exit(1);
});
