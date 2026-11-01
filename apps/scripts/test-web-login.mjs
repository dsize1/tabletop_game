// Web 跨端登录端到端测试（ADR-0031）。
//
// 覆盖：
//   1) 申请短链 → 注册（暂存）→ 4 位码 → 错人确认(mismatch) → 本人确认 → 换票（Cookie HttpOnly）
//   2) 响应体不含正式 token；链接二次访问返回 used
//   3) 注册码错误 3 次锁定（30 分钟）
//   4) 重置注册码（mode=reset）：确认成功但不跳转、不换票
//   5) 未确认的注册不落库（错链注册不污染）→ 重新申请仍为 need_register
//   6) 中台 scope：白名单/非白名单（依赖 server PROPS_ADMIN_WHITELIST）
//
// 前提：Redis 与 **server / web 已在运行**（可用 `npm run dev:up:noqq`）：
//   SERVER_URL=http://127.0.0.1:3000 WEB_BASE=http://127.0.0.1:18080
// 未就绪 → 打印 skip 并以 0 退出（CI 不阻塞）。
//
// 用法：node apps/scripts/test-web-login.mjs

const SERVER = (process.env.SERVER_URL ?? "http://127.0.0.1:3000").replace(/\/+$/, "");
const WEB = (process.env.WEB_BASE ?? "http://127.0.0.1:18080").replace(/\/+$/, "");
const PASSCODE = "Ab3dE9";
const PASSCODE2 = "Zz9zZ9";

let failed = 0;

function log(msg) {
  console.log(`[web-login] ${msg}`);
}
function assert(cond, msg) {
  if (!cond) {
    failed += 1;
    console.error(`❌ ${msg}`);
  }
}
async function skip(reason) {
  console.log(`[web-login] skip: ${reason}`);
  process.exit(0);
}

async function reachable(url) {
  try {
    const r = await fetch(url, { signal: AbortSignal.timeout(2500) });
    return r.ok;
  } catch {
    return false;
  }
}

async function json(url, init) {
  const r = await fetch(url, init);
  const text = await r.text();
  let body = null;
  try {
    body = text ? JSON.parse(text) : null;
  } catch {
    body = { raw: text };
  }
  return { status: r.status, body, headers: r.headers };
}

const post = (url, body) =>
  json(url, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  });

const stamp = Date.now();
const GROUP = `e2e-group-${stamp}`;
const OPENID = `e2e-openid-${stamp}`;
const OTHER = `e2e-other-${stamp}`;
const ROOM = `e2e-room-${stamp}`;

function target(openid) {
  return `/tabletop_game/sanguosha/player/${encodeURIComponent(openid)}`;
}

async function issueLink(openid, extra = {}) {
  const r = await post(`${SERVER}/internal/auth/web/link`, {
    group_id: GROUP,
    openid,
    display_name: "测试玩家",
    scope: "game",
    target: target(openid),
    room_id: ROOM,
    ...extra,
  });
  assert(r.status === 200, `申请链接应 200，实际 ${r.status} ${JSON.stringify(r.body)}`);
  return r.body;
}

async function status(t, wait = 0) {
  return json(`${WEB}/login/status?t=${encodeURIComponent(t)}&wait=${wait}`);
}

/** 抽出 Set-Cookie 里的指定 cookie 值（web 的 confirm 响应）。 */
function cookieFrom(headers, name) {
  const all = headers.getSetCookie?.() ?? [];
  for (const c of all) {
    if (c.startsWith(`${name}=`)) {
      return c.split(";")[0].slice(name.length + 1);
    }
  }
  return "";
}

async function main() {
  if (!(await reachable(`${SERVER}/health`))) {
    await skip(`server 未就绪（${SERVER}）`);
  }
  if (!(await reachable(`${WEB}/health`))) {
    await skip(`web 未就绪（${WEB}）`);
  }

  // ---- 1) 首次进入：未注册 ----
  const link = await issueLink(OPENID);
  assert(link.stage === "need_register", `新用户应 need_register，实际 ${link.stage}`);
  let st = await status(link.token);
  assert(st.status === 200, `status 应 200，实际 ${st.status}`);
  assert(st.body.display_name === "测试玩家", `应回显群昵称，实际 ${st.body.display_name}`);
  assert(st.body.registered === false, "应标记未注册");
  // 开局摘要：房间存在时返回；本 e2e 用假 room_id，故只校验字段类型（存在则必须是字符串）
  if (st.body.game !== undefined) {
    assert(typeof st.body.game?.summary === "string", "开局摘要应为字符串");
  } else {
    log("房间未落库（假 room_id）→ 跳过开局摘要内容断言");
  }

  // ---- 2) 注册码校验：格式 / 一致性 ----
  let r = await post(`${WEB}/login/register`, {
    t: link.token,
    passcode: "abc",
    passcode_confirm: "abc",
  });
  assert(r.status === 400, `短注册码应 400，实际 ${r.status}`);
  r = await post(`${WEB}/login/register`, {
    t: link.token,
    passcode: PASSCODE,
    passcode_confirm: PASSCODE2,
  });
  assert(r.status === 400, `两次不一致应 400，实际 ${r.status}`);

  // ---- 3) 注册成功 → 4 位码 ----
  r = await post(`${WEB}/login/register`, {
    t: link.token,
    passcode: PASSCODE,
    passcode_confirm: PASSCODE,
  });
  assert(r.status === 200, `注册应 200，实际 ${r.status} ${JSON.stringify(r.body)}`);
  const code = String(r.body.code ?? "");
  assert(/^\d{4}$/.test(code), `验证码应为 4 位数字，实际 ${code}`);
  assert(!JSON.stringify(r.body).includes("token"), "注册响应不得包含正式 token");

  // ---- 4) 错人确认 → mismatch（不确认） ----
  let v = await post(`${SERVER}/internal/auth/web/verify`, {
    group_id: GROUP,
    code,
    openid: OTHER,
  });
  assert(v.body.result === "mismatch", `他人确认应 mismatch，实际 ${v.body.result}`);
  st = await status(link.token);
  assert(st.body.stage === "code", `错人确认后仍应停在 code，实际 ${st.body.stage}`);

  // ---- 5) 本人确认 → 换票 ----
  v = await post(`${SERVER}/internal/auth/web/verify`, { group_id: GROUP, code, openid: OPENID });
  assert(v.body.result === "confirmed", `本人确认应 confirmed，实际 ${v.body.result}`);
  const conf = await json(`${WEB}/login/confirm?t=${encodeURIComponent(link.token)}&wait=0`);
  assert(conf.status === 200, `confirm 应 200，实际 ${conf.status}`);
  assert(conf.body.status === "confirmed", `confirm 应 confirmed，实际 ${conf.body.status}`);
  assert(conf.body.mode === "login", `confirm mode 应为 login，实际 ${conf.body.mode}`);
  assert(conf.body.target === target(OPENID), `confirm target 不匹配：${conf.body.target}`);
  assert(!("token" in conf.body), "confirm 响应体不得包含 token（只走 Set-Cookie）");
  const gameToken = cookieFrom(conf.headers, "tg_game");
  assert(Boolean(gameToken), "confirm 应写 tg_game Cookie");
  const rawCookie = (conf.headers.getSetCookie?.() ?? []).find((c) => c.startsWith("tg_game=")) ?? "";
  assert(/HttpOnly/i.test(rawCookie), `Cookie 必须 HttpOnly：${rawCookie}`);
  assert(/SameSite=Lax/i.test(rawCookie), `Cookie 必须 SameSite=Lax：${rawCookie}`);

  // ---- 6) 换到的 token 可用（whoami） ----
  const who = await json(`${SERVER}/auth/web/whoami?token=${encodeURIComponent(gameToken)}`);
  assert(who.status === 200 && who.body.openid === OPENID, `whoami 应返回本人：${JSON.stringify(who.body)}`);

  // ---- 7) 链接二次访问 → used ----
  const again = await json(`${WEB}/login/confirm?t=${encodeURIComponent(link.token)}&wait=0`);
  assert(again.body.status === "used", `二次访问应 used，实际 ${again.body.status}`);

  // ---- 8) 已注册 → 登录（need_login），并且锁定策略生效 ----
  const link2 = await issueLink(OPENID);
  assert(link2.stage === "need_login", `已注册应 need_login，实际 ${link2.stage}`);
  for (let i = 0; i < 3; i += 1) {
    r = await post(`${WEB}/login/login`, { t: link2.token, passcode: "bad000" });
    assert(r.status === 401, `错误注册码应 401（第 ${i + 1} 次），实际 ${r.status}`);
  }
  r = await post(`${WEB}/login/login`, { t: link2.token, passcode: PASSCODE });
  assert(r.status === 423, `锁定后应 423，实际 ${r.status} ${JSON.stringify(r.body)}`);

  // 等锁定不现实（30 分钟）→ 用另一个 openid 验证正常登录路径
  const OPENID2 = `${OPENID}-b`;
  const link3 = await issueLink(OPENID2);
  r = await post(`${WEB}/login/register`, {
    t: link3.token,
    passcode: PASSCODE2,
    passcode_confirm: PASSCODE2,
  });
  assert(r.status === 200, `第二个用户注册应 200，实际 ${r.status}`);
  const code3 = String(r.body.code ?? "");
  v = await post(`${SERVER}/internal/auth/web/verify`, {
    group_id: GROUP,
    code: code3,
    openid: OPENID2,
  });
  assert(v.body.result === "confirmed", "第二个用户确认应 confirmed");
  const conf3 = await json(`${WEB}/login/confirm?t=${encodeURIComponent(link3.token)}&wait=0`);
  assert(conf3.body.status === "confirmed", "第二个用户 confirm 应 confirmed");

  // ---- 9) 重置注册码：确认后不换票、不跳转 ----
  const reset = await issueLink(OPENID2, { mode: "reset" });
  assert(reset.stage === "need_register", `重置应进入设置新注册码，实际 ${reset.stage}`);
  r = await post(`${WEB}/login/register`, {
    t: reset.token,
    passcode: PASSCODE,
    passcode_confirm: PASSCODE,
  });
  assert(r.status === 200, `重置设置新码应 200，实际 ${r.status}`);
  const resetCode = String(r.body.code ?? "");
  v = await post(`${SERVER}/internal/auth/web/verify`, {
    group_id: GROUP,
    code: resetCode,
    openid: OPENID2,
  });
  assert(v.body.result === "confirmed", "重置确认应 confirmed");
  const confReset = await json(`${WEB}/login/confirm?t=${encodeURIComponent(reset.token)}&wait=0`);
  assert(confReset.body.status === "confirmed", `重置 confirm 应 confirmed，实际 ${confReset.body.status}`);
  assert(confReset.body.mode === "reset", "重置 confirm mode 应为 reset");
  assert(!("token" in confReset.body), "重置不应发放 token");
  assert(!cookieFrom(confReset.headers, "tg_game"), "重置不应写 Cookie");
  // 用新注册码可登录
  const link4 = await issueLink(OPENID2);
  assert(link4.stage === "need_login", "重置后仍应 need_login");
  r = await post(`${WEB}/login/login`, { t: link4.token, passcode: PASSCODE });
  assert(r.status === 200, `重置后的新注册码应可登录，实际 ${r.status} ${JSON.stringify(r.body)}`);

  // ---- 10) 未确认的注册不落库（错链注册不污染目标账号） ----
  const OPENID3 = `${OPENID}-c`;
  const link5 = await issueLink(OPENID3);
  r = await post(`${WEB}/login/register`, {
    t: link5.token,
    passcode: PASSCODE,
    passcode_confirm: PASSCODE,
  });
  assert(r.status === 200, "临时注册应 200（仅暂存）");
  const link6 = await issueLink(OPENID3);
  assert(link6.stage === "need_register", `未确认的注册不应落库（应仍 need_register），实际 ${link6.stage}`);

  // ---- 11) 中台 scope：非白名单 403 ----
  const props = await post(`${SERVER}/internal/auth/web/link`, {
    group_id: GROUP,
    openid: OPENID,
    display_name: "测试玩家",
    scope: "props",
    target: "/props-admin/",
  });
  assert(
    props.status === 403 || props.status === 200,
    `中台链接应 200/403，实际 ${props.status}`,
  );
  log(`中台 scope 结果：${props.status}（白名单配置决定）`);

  if (failed) {
    console.error(`\n❌ web 登录 e2e 失败：${failed} 项断言未通过`);
    process.exit(1);
  }
  console.log("\n✅ web 登录 e2e 通过（注册/确认/换票/used/锁定/重置/不污染/中台）");
}

main().catch((err) => {
  console.error("[web-login] 异常:", err);
  process.exit(1);
});
