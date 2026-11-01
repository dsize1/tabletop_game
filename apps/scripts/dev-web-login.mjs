#!/usr/bin/env node
// 本地开发辅助：签发 Web 跨端登录链接，并在本机「模拟群内 #验证码#NNNN」完成确认。
//
// 为什么需要它：真实流程要玩家在 QQ 群里回复验证码；本地没有 QQ 消息回路时无法完成，
// 于是用本脚本调用 **server 内部接口** `/internal/auth/web/verify` 代替机器人回执。
// 只在开发机使用（server 内部接口仅 127.0.0.1 可达）。
//
// 用法：
//   npm run props:login                     # 中台（scope=props，target=/props-admin/）
//   npm run dev:web-login -- --scope game --room <room_id>
//   可选：--openid <id>、--no-auto（手动确认）、--reset（重置注册码：确认后不跳转、不发 token）
//   node apps/scripts/dev-web-login.mjs --confirm 1234 --openid <id>   # 手动确认某码
//
// 前置：`npm run dev:up`（Redis + server + web）；openid 必须在 `DEV_WHITELIST`
//      （中台用 `PROPS_ADMIN_WHITELIST`，未设置时回落 `DEV_WHITELIST`）。

import { existsSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";

const args = process.argv.slice(2);
function arg(name, fallback) {
  const i = args.indexOf(`--${name}`);
  if (i < 0) return fallback;
  const v = args[i + 1];
  return v && !v.startsWith("--") ? v : true;
}
const has = (name) => args.includes(`--${name}`);

const SERVER = String(arg("server", process.env.SERVER_URL ?? "http://127.0.0.1:3000")).replace(/\/+$/, "");
const WEB = String(arg("web", process.env.WEB_BASE ?? "http://127.0.0.1:18080")).replace(/\/+$/, "");
const SCOPE = String(arg("scope", "props"));
const ROOM = arg("room", undefined);
const OPENID = arg("openid", process.env.DEV_OPENID);

/** 从最近一条群消息里取 member_openid（本地没有 QQ 时最方便）。 */
function latestOpenidFromDb() {
  const dbPath = process.env.SQLITE_PATH ?? "data/tabletop.db";
  if (!existsSync(dbPath)) return undefined;
  try {
    const db = new DatabaseSync(dbPath, { readOnly: true });
    const rows = db
      .prepare("SELECT raw_payload FROM group_messages WHERE raw_payload LIKE '%member_openid%' ORDER BY id DESC LIMIT 5")
      .all();
    db.close();
    for (const r of rows) {
      try {
        const a = JSON.parse(r.raw_payload)?.raw?.author;
        if (a?.member_openid) return a.member_openid;
      } catch {
        /* ignore */
      }
    }
  } catch {
    /* ignore */
  }
  return undefined;
}

const post = (url, body) =>
  fetch(url, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  });

async function main() {
  const confirmOnly = arg("confirm", undefined);
  const openid = OPENID ?? latestOpenidFromDb();

  if (confirmOnly && typeof confirmOnly === "string") {
    if (!openid) {
      console.error("❌ 缺少 --openid（或设置 DEV_OPENID）");
      process.exit(1);
    }
    const r = await post(`${SERVER}/internal/auth/web/verify`, {
      group_id: String(arg("group", "dev")),
      code: confirmOnly,
      openid,
    });
    const body = await r.json().catch(() => ({}));
    console.log(`验证码 ${confirmOnly} → ${body.result ?? r.status}`);
    process.exit(body.result === "confirmed" ? 0 : 1);
  }

  if (!openid) {
    console.error("❌ 找不到 openid：请加 --openid <member_openid>（或设置 DEV_OPENID）");
    process.exit(1);
  }
  const target = String(arg("target", SCOPE === "props" ? "/props-admin/" : "/"));
  const displayName = String(arg("name", "开发"));
  const groupId = String(arg("group", "dev-group"));

  const r = await post(`${SERVER}/internal/auth/web/link`, {
    group_id: groupId,
    openid,
    display_name: displayName,
    scope: SCOPE,
    target,
    ...(ROOM ? { room_id: ROOM } : {}),
    ...(has("reset") ? { mode: "reset" } : {}),
  });
  const link = await r.json().catch(() => ({}));
  if (!r.ok) {
    console.error(`❌ 申请登录链接失败：${r.status} ${link.error ?? ""}`);
    if (r.status === 403) {
      console.error(
        "   该 openid 不在白名单：把它加入 apps/agent-gateway/.env 的 DEV_WHITELIST" +
          "（中台可用 PROPS_ADMIN_WHITELIST），然后重启 dev:up。",
      );
    }
    if (r.status === 400 && String(link.error ?? "").includes("room_id")) {
      console.error("   scope=game 需要 --room <room_id>（当前群对局 room id）。");
    }
    process.exit(1);
  }

  const url = `${WEB}/login?t=${encodeURIComponent(link.token)}`;
  console.log("== Web 跨端登录（本地开发）==");
  console.log(`openid: ${openid}`);
  console.log(`stage : ${link.stage}（need_register=先设注册码；need_login=输注册码）`);
  console.log(`链接  : ${url}`);
  console.log("→ 用浏览器打开上面的链接，设置/输入 6 位注册码。");
  console.log("→ 页面出现 4 位验证码后，本脚本会模拟你在群里回复 `@机器人 #验证码#NNNN`。\n");

  const deadline = Date.now() + 10 * 60 * 1000;
  let lastStage = "";
  let done = false;
  while (Date.now() < deadline) {
    const sr = await fetch(
      `${SERVER}/auth/web/status?token=${encodeURIComponent(link.token)}`,
    );
    if (!sr.ok) {
      console.log("链接已过期（10 分钟）→ 请重新运行本脚本。");
      break;
    }
    const st = await sr.json();
    if (st.stage !== lastStage) {
      lastStage = st.stage;
      console.log(`[状态] ${st.stage}${st.code ? `（验证码 ${st.code}）` : ""}`);
    }
    if (st.consumed) {
      console.log("✅ 已换票（本脚本结束）。若页面未跳转，刷新页面即可。");
      done = true;
      break;
    }
    if (st.stage === "confirmed") {
      // 让页面自己完成 confirm 换票（脚本不抢），再等 consumed
      console.log("✅ 群内确认完成 → 回到浏览器，页面会自动进入。");
      done = true;
      break;
    }
    if (st.stage === "code" && st.code && !has("no-auto")) {
      const vr = await post(`${SERVER}/internal/auth/web/verify`, {
        group_id: groupId,
        code: st.code,
        openid,
      });
      const vb = await vr.json().catch(() => ({}));
      console.log(`[模拟群回复] @机器人 #验证码#${st.code} → ${vb.result ?? vr.status}`);
    }
    await new Promise((r2) => setTimeout(r2, 1000));
  }
  if (!done && Date.now() >= deadline) console.log("⏱ 等待超时（10 分钟）。");
}

main().catch((err) => {
  console.error("[dev-web-login] 异常:", err);
  process.exit(1);
});
