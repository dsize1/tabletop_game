#!/usr/bin/env node
// 抓取三国杀官网武将数据（公开页面），生成道具中台导入用的 JSON。
//
// 来源：https://www.sanguosha.com/hero （列表页内嵌 `const Listdata = {...}`）
//       https://www.sanguosha.com/hero/{gid} （详情页：技能名 + 技能描述）
// 说明：
// - 仅抓取公开前台内容，遵守 robots.txt（/api/ 不抓）；默认串行/低并发 + 间隔，勿压站。
// - 官网数据版权归官方所有：产物仅用于内部开发/初始化，上线前需确认授权（见开发计划风险）。
// - 列表页 `initial_hp` 全为 0（表示未提供），身份局初始体力 = `hp`（主公 +1 由引擎处理）。
// - `gender` 官网未提供，导入时留 `unknown`，后续由中台补。
//
// 用法：
//   node apps/scripts/crawl-sanguosha-heroes.mjs                 # 全量（默认 4 并发、120ms 间隔）
//   node apps/scripts/crawl-sanguosha-heroes.mjs --limit 20      # 抽样
//   node apps/scripts/crawl-sanguosha-heroes.mjs --out docs/games/sanguosha/data/heroes.json
//   node apps/scripts/crawl-sanguosha-heroes.mjs --refresh       # 忽略缓存重抓
//
// 缓存：data/sanguosha-cache/（data/ 已在 .gitignore，缓存不入库）

import { mkdir, readFile, writeFile } from "node:fs/promises";
import { existsSync } from "node:fs";
import { dirname, resolve } from "node:path";
import vm from "node:vm";

const BASE = "https://www.sanguosha.com";
const LIST_URL = `${BASE}/hero`;
const CACHE_DIR = resolve("data", "sanguosha-cache");
const DEFAULT_OUT = resolve("docs", "games", "sanguosha", "data", "heroes.json");
const UA =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36";

function parseArgs(argv) {
  const opts = {
    out: DEFAULT_OUT,
    limit: 0,
    concurrency: 4,
    delay: 120,
    refresh: false,
  };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--out") opts.out = resolve(argv[++i]);
    else if (a === "--limit") opts.limit = Number(argv[++i]) || 0;
    else if (a === "--concurrency") opts.concurrency = Math.max(1, Number(argv[++i]) || 4);
    else if (a === "--delay") opts.delay = Math.max(0, Number(argv[++i]) || 0);
    else if (a === "--refresh") opts.refresh = true;
    else if (a === "--help" || a === "-h") {
      console.log("用法：node apps/scripts/crawl-sanguosha-heroes.mjs [--out path] [--limit n] [--concurrency n] [--delay ms] [--refresh]");
      process.exit(0);
    } else {
      console.error(`未知参数：${a}`);
      process.exit(1);
    }
  }
  return opts;
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function fetchText(url, { retries = 2 } = {}) {
  let lastErr;
  for (let attempt = 0; attempt <= retries; attempt++) {
    try {
      const res = await fetch(url, {
        headers: { "user-agent": UA, accept: "text/html,application/xhtml+xml" },
        signal: AbortSignal.timeout(30_000),
      });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      return await res.text();
    } catch (err) {
      lastErr = err;
      if (attempt < retries) await sleep(500 * (attempt + 1));
    }
  }
  throw new Error(`抓取失败 ${url}: ${lastErr?.message ?? lastErr}`);
}

/** 从列表页内嵌脚本提取 Listdata（不解 HTML，直接执行可信的官网内联脚本）。 */
function extractListdata(html) {
  const i = html.indexOf("const Listdata");
  if (i < 0) throw new Error("列表页未找到 const Listdata");
  const s = html.lastIndexOf("<script", i);
  const start = html.indexOf(">", s) + 1;
  const end = html.indexOf("</script>", i);
  const code = html.slice(start, end);
  const ctx = { console: { log: () => {} } };
  vm.createContext(ctx);
  vm.runInContext(`${code}\n;globalThis.__Listdata = Listdata;`, ctx);
  return ctx.__Listdata;
}

/** 极简 HTML 实体解码 + 标签清理。 */
function decodeText(s) {
  return s
    .replace(/<br\s*\/?>/gi, "\n")
    .replace(/<[^>]+>/g, "")
    .replace(/&nbsp;/g, " ")
    .replace(/&amp;/g, "&")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    .replace(/&#x([0-9a-f]+);/gi, (_, h) => String.fromCodePoint(parseInt(h, 16)))
    .replace(/&#(\d+);/g, (_, d) => String.fromCodePoint(Number(d)))
    .replace(/[ \t]+\n/g, "\n")
    .replace(/\n{3,}/g, "\n\n")
    .trim();
}

/** 详情页：技能名 + 技能描述。 */
function parseDetail(html) {
  const skillNames = [
    ...html.matchAll(/<div class="character-tab(?!s)[^"]*">([\s\S]*?)<\/div>/g),
  ].map((m) => decodeText(m[1]));

  const block = html.match(/<p class="skill-text">([\s\S]*?)<\/p>/);
  const descs = block
    ? [...block[1].matchAll(/<span[^>]*>([\s\S]*?)<\/span>/g)].map((m) => decodeText(m[1]))
    : [];

  const skills = [];
  const n = Math.max(skillNames.length, descs.length);
  for (let i = 0; i < n; i++) {
    const name = skillNames[i] ?? `技能${i + 1}`;
    const description = descs[i] ?? "";
    skills.push({ name, description, type_guess: guessSkillType(description) });
  }
  return skills;
}

function guessSkillType(desc) {
  if (!desc) return null;
  if (/^主公技/.test(desc)) return "lord";
  if (/^锁定技/.test(desc)) return "locked";
  if (/^限定技/.test(desc)) return "limited";
  if (/^觉醒技/.test(desc)) return "awakening";
  return "normal";
}

async function detail(gid, { refresh }) {
  const cachePath = resolve(CACHE_DIR, `${gid}.html`);
  if (!refresh && existsSync(cachePath)) {
    return readFile(cachePath, "utf8");
  }
  const html = await fetchText(`${BASE}/hero/${gid}`);
  await writeFile(cachePath, html, "utf8");
  return html;
}

async function main() {
  const opts = parseArgs(process.argv.slice(2));
  await mkdir(CACHE_DIR, { recursive: true });
  await mkdir(dirname(opts.out), { recursive: true });

  console.log(`[crawl] 列表页 ${LIST_URL}`);
  const listHtml = await fetchText(LIST_URL);
  const listdata = extractListdata(listHtml);

  const packs = Object.keys(listdata).filter((k) => k !== "全部");
  const all = packs.flatMap((pack) =>
    listdata[pack].map((h) => ({ ...h, pack })),
  );
  const byGid = new Map();
  for (const h of all) byGid.set(h.gid, h);
  let heroes = [...byGid.values()].sort((a, b) => a.gid - b.gid);
  if (opts.limit > 0) heroes = heroes.slice(0, opts.limit);
  console.log(`[crawl] 列表 ${byGid.size} 位，本次抓取 ${heroes.length} 位（并发 ${opts.concurrency}、间隔 ${opts.delay}ms）`);

  const out = [];
  let done = 0;
  let fail = 0;
  const queue = [...heroes];

  async function worker() {
    while (queue.length > 0) {
      const h = queue.shift();
      try {
        const html = await detail(h.gid, opts);
        const skills = parseDetail(html);
        out.push({
          gid: h.gid,
          name: h.name,
          realname: h.realname ?? h.name,
          pack: h.pack,
          kingdom: h.figure,
          kingdom_code: h.figure_py,
          hp: h.hp,
          initial_hp: h.initial_hp,
          quality: h.quality ?? null,
          biography: h.biography ?? "",
          characteristic: h.characteristic ?? "",
          avatar: h.avatar ?? null,
          cover: h.cover ?? null,
          skills,
        });
        done++;
      } catch (err) {
        fail++;
        console.error(`[crawl] ${h.gid} ${h.name} 失败: ${err.message}`);
      }
      if ((done + fail) % 25 === 0) {
        console.log(`[crawl] 进度 ${done + fail}/${heroes.length}（成功 ${done}，失败 ${fail}）`);
      }
      if (opts.delay > 0) await sleep(opts.delay + Math.floor(Math.random() * 80));
    }
  }

  await Promise.all(Array.from({ length: opts.concurrency }, () => worker()));
  out.sort((a, b) => a.gid - b.gid);

  const payload = {
    source: LIST_URL,
    fetched_at: new Date().toISOString(),
    count: out.length,
    packs: Object.fromEntries(packs.map((p) => [p, listdata[p].length])),
    heroes: out,
  };
  await writeFile(opts.out, JSON.stringify(payload), "utf8");
  console.log(`[crawl] 完成：${out.length} 位，失败 ${fail}；输出 ${opts.out}`);
  if (fail > 0) process.exitCode = 2;
}

main().catch((err) => {
  console.error("[crawl] 失败:", err);
  process.exit(1);
});
