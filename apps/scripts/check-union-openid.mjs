#!/usr/bin/env node
// 探针：平台是否已打通「跨应用统一身份」（`union_openid`）。
//
// 背景（ADR-0031）：QQ 机器人事件里的 `union_openid` 目前恒为空，需平台运营
// 「特殊关联应用」后才有值；一旦有值，就可以把 QQ 互联扫码登录接入为 Web 鉴权。
//
// 用法：
//   node apps/scripts/check-union-openid.mjs [--db data/tabletop.db] [--limit 5]
//
// 输出：群事件总数 / 含非空 union_openid 的条数 / 占比 / 样例。
// 退出码始终 0（探针，不阻塞 CI）。

import { existsSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";

function arg(name, fallback) {
  const i = process.argv.indexOf(`--${name}`);
  if (i < 0) return fallback;
  const v = process.argv[i + 1];
  return v && !v.startsWith("--") ? v : true;
}

const dbPath = String(arg("db", process.env.SQLITE_PATH ?? "data/tabletop.db"));
const limit = Number(arg("limit", 5));

if (!existsSync(dbPath)) {
  console.log(`[union-openid] 跳过：找不到数据库 ${dbPath}`);
  process.exit(0);
}

let db;
try {
  db = new DatabaseSync(dbPath, { readOnly: true });
} catch (err) {
  console.log(`[union-openid] 跳过：无法打开数据库（${err.message}）`);
  process.exit(0);
}

let total = 0;
let withUnion = 0;
const samples = [];
try {
  const rows = db.prepare("SELECT raw_payload FROM group_messages").all();
  for (const row of rows) {
    const raw = row.raw_payload;
    if (!raw || !raw.includes("member_openid")) continue;
    total += 1;
    let parsed;
    try {
      parsed = JSON.parse(raw);
    } catch {
      continue;
    }
    const author = parsed?.raw?.author;
    const union = typeof author?.union_openid === "string" ? author.union_openid.trim() : "";
    if (union) {
      withUnion += 1;
      if (samples.length < limit) {
        samples.push({
          group: parsed?.raw?.group_openid,
          member: author?.member_openid,
          union,
          role: author?.member_role,
        });
      }
    }
  }
} finally {
  db.close();
}

const pct = total ? ((withUnion / total) * 100).toFixed(2) : "0.00";
console.log(`[union-openid] 群事件 ${total} 条；非空 union_openid ${withUnion} 条（${pct}%）`);
if (samples.length) {
  console.log("[union-openid] 样例：");
  for (const s of samples) console.log(`  - group=${s.group} member=${s.member} union=${s.union} role=${s.role ?? "-"}`);
} else {
  console.log("[union-openid] 结论：平台尚未对本站开放跨应用身份（扫码登录仍阻塞，继续走跨端确认流程）。");
}
