// Web 跨端登录页（ADR-0031）。
//
// 流程：群内链接（短效 t）→ 页面展示昵称/开局信息 → 注册或输注册码 →
//      反显 4 位验证码（一键复制 `@机器人 #验证码#NNNN`）→ 长轮询确认 →
//      浏览器拿到 HttpOnly Cookie → 2 秒过渡 → 跳转目标路由。
//
// 安全：短效 token 只在页面 URL 与请求体内，不写 localStorage；正式 token 只经
//      `Set-Cookie`（HttpOnly）下发，JS 读不到，也不出现在 URL。
//
// 页面为单文件渲染（无外部依赖），脚本内联；CSP 由 web 站点统一策略约束。

import { escapeHtml } from "./format.js";

const STYLES = `
:root { color-scheme: dark; }
* { box-sizing: border-box; }
body {
  margin: 0; min-height: 100vh;
  background: radial-gradient(1200px 600px at 50% -10%, #1b2530 0%, #0b0f14 60%);
  color: #e6edf3;

  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", "PingFang SC", "Microsoft YaHei", sans-serif;
  display: flex; align-items: flex-start; justify-content: center;
  padding: 24px 12px 48px;
}
.card {
  width: 100%; max-width: 460px; background: #141a21; border: 1px solid #2b3644;
  border-radius: 16px; padding: 18px 18px 20px; box-shadow: 0 12px 40px rgba(0,0,0,.45);
}
.brand { display: flex; align-items: center; gap: 8px; font-size: 13px; color: #8b98a8; letter-spacing: .4px; }
.brand b { color: #e3b341; font-weight: 700; }
h1 { font-size: 19px; margin: 10px 0 4px; }
.who { font-size: 15px; margin: 0 0 2px; }
.who b { color: #e3b341; }
.info { font-size: 12.5px; color: #8b98a8; line-height: 1.6; margin-top: 6px; word-break: break-all; }
.panel { margin-top: 14px; padding: 14px; background: #10161d; border: 1px solid #263041; border-radius: 12px; }
.panel[hidden] { display: none; }
label { display: block; font-size: 12.5px; color: #9aa7b6; margin: 0 0 6px; }
input[type="password"], input[type="text"] {
  width: 100%; padding: 10px 12px; font-size: 17px; letter-spacing: 3px;
  background: #0b1117; color: #e6edf3; border: 1px solid #334155; border-radius: 10px;
  font-family: ui-monospace, Consolas, monospace; outline: none;
}
input:focus { border-color: #e3b341; }
.row { margin-bottom: 10px; }
button {
  appearance: none; border: 0; border-radius: 10px; padding: 11px 14px;
  font-size: 15px; font-weight: 700; cursor: pointer; width: 100%;
  background: #e3b341; color: #1a1206;
}
button.secondary { background: #1f2a36; color: #cfe0f0; border: 1px solid #334155; }
button:disabled { opacity: .55; cursor: default; }
.hint { font-size: 12px; color: #7d8b9b; margin-top: 8px; line-height: 1.55; }
.err { font-size: 13px; color: #f0883e; margin-top: 10px; min-height: 18px; white-space: pre-wrap; }
.code {
  font-family: ui-monospace, Consolas, monospace; font-size: 40px; letter-spacing: 12px;
  text-align: center; padding: 14px 0 6px; color: #e3b341; font-weight: 700;
}
.cmd {
  font-family: ui-monospace, Consolas, monospace; font-size: 14px; text-align: center;
  background: #0b1117; border: 1px dashed #3b4a5e; border-radius: 10px;
  padding: 10px; margin: 6px 0 10px; word-break: break-all; color: #cfe0f0;
}
.ok { font-size: 14px; color: #57d38c; text-align: center; padding: 10px 0; }
.center { text-align: center; }
.muted { color: #6e7b8a; font-size: 12px; }
a { color: #7cb6f2; }
@media (max-width: 420px) {
  .code { font-size: 34px; letter-spacing: 8px; }
}
`;

/** 登录页（内联脚本；不使用外部 bundle，避免与游戏页构建耦合）。 */
export function renderLoginPage(opts: { token: string; siteName?: string }): string {
  const site = opts.siteName ?? "桌游助手";
  return `<!doctype html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<meta name="robots" content="noindex, nofollow">
<title>登录 · ${escapeHtml(site)}</title>
<style>${STYLES}</style>
</head>
<body>
<main class="card">
  <div class="brand">🎲 <b>${escapeHtml(site)}</b> · 身份确认</div>
  <h1 id="title">正在读取登录信息…</h1>
  <p class="who" id="who" hidden></p>
  <div class="info" id="info" hidden></div>
  <div class="err" id="err" role="alert" aria-live="polite"></div>

  <section class="panel" id="step-loading">
    <div class="hint">请稍候，正在校验链接…</div>
  </section>

  <section class="panel" id="step-register" hidden>
    <div class="row">
      <label for="p1">设置注册码（6 位数字或字母，仅你自己知道）</label>
      <input id="p1" type="password" inputmode="latin" autocomplete="new-password" maxlength="6" placeholder="6 位 0-9a-zA-Z">
    </div>
    <div class="row">
      <label for="p2">再输入一次</label>
      <input id="p2" type="password" inputmode="latin" autocomplete="new-password" maxlength="6" placeholder="与上面一致">
    </div>
    <button id="btn-register">注册并获取验证码</button>
    <div class="hint">注册码用于以后进入本群对局的网页；忘记可在群里发「#重置注册码」。</div>
  </section>

  <section class="panel" id="step-login" hidden>
    <div class="row">
      <label for="p3">输入你的注册码</label>
      <input id="p3" type="password" inputmode="latin" autocomplete="current-password" maxlength="6" placeholder="6 位注册码">
    </div>
    <button id="btn-login">输入注册码</button>
    <button class="secondary" id="btn-reset" style="margin-top:8px">忘记注册码？</button>
    <div class="hint">忘记可点上方按钮，回群发「#重置注册码」重新设置。</div>
  </section>

  <section class="panel" id="step-code" hidden>
    <div class="center muted">请在 QQ 群里发送下面这条消息（用你自己的 QQ 号）</div>
    <div class="code" id="code">0000</div>
    <div class="cmd" id="cmd">@机器人 #验证码#0000</div>
    <button id="btn-copy">一键复制</button>
    <div class="hint">发送后回到本页面，会自动进入游戏。验证码 3 分钟内有效，别转发给他人。</div>
  </section>

  <section class="panel" id="step-confirmed" hidden>
    <div class="ok">✅ 已确认身份</div>
    <div class="hint center" id="confirm-hint">正在进入…（2 秒）</div>
  </section>

  <section class="panel" id="step-done" hidden>
    <div class="ok">✅ 注册码已重置</div>
    <div class="hint">可以关闭本页了。下次进入请用新注册码。</div>
  </section>

  <section class="panel" id="step-fatal" hidden>
    <div class="hint" id="fatal-text"></div>
  </section>
</main>
<script>
(function () {
  "use strict";
  var TOKEN = ${JSON.stringify(opts.token)};
  var state = null;
  var pollAbort = null;

  function $(id) { return document.getElementById(id); }
  function show(id) {
    ["step-loading","step-register","step-login","step-code","step-confirmed","step-done","step-fatal"].forEach(function (k) {
      var el = $(k); if (el) el.hidden = (k !== id);
    });
  }
  function setErr(msg) { $("err").textContent = msg || ""; }
  function fatal(msg) { $("fatal-text").textContent = msg; show("step-fatal"); $("title").textContent = "链接不可用"; }

  async function api(path, body) {
    var res = await fetch(path, {
      method: body === undefined ? "GET" : "POST",
      headers: body === undefined ? { "accept": "application/json" } : { "content-type": "application/json", "accept": "application/json" },
      body: body === undefined ? undefined : JSON.stringify(body),
      credentials: "same-origin"
    });
    var data = null;
    try { data = await res.json(); } catch (e) { data = null; }
    return { ok: res.ok, status: res.status, data: data || {} };
  }

  function renderCode(code) {
    $("code").textContent = code;
    $("cmd").textContent = "@机器人 #验证码#" + code;
    show("step-code");
    poll();
  }

  async function poll() {
    if (pollAbort) { pollAbort.abort(); }
    var ctrl = new AbortController();
    pollAbort = ctrl;
    for (;;) {
      if (ctrl.signal.aborted) return;
      var r;
      try {
        r = await fetch("/login/confirm?t=" + encodeURIComponent(TOKEN), {
          headers: { "accept": "application/json" }, credentials: "same-origin", signal: ctrl.signal
        });
      } catch (e) {
        if (ctrl.signal.aborted) return;
        await new Promise(function (r2) { setTimeout(r2, 2000); });
        continue;
      }
      var data = null;
      try { data = await r.json(); } catch (e) { data = null; }
      if (!r.ok) {
        fatal((data && data.error) || "链接已失效，请回群里重新获取（#局面 / #网页 / #中台）");
        return;
      }
      if (data.status === "confirmed") {
        show("step-confirmed");
        $("confirm-hint").textContent = data.mode === "reset" ? "注册码已重设" : "正在进入…（2 秒）";
        if (data.mode === "reset") { show("step-done"); return; }
        setTimeout(function () {
          location.replace(data.target || "/");
        }, 2000);
        return;
      }
      if (data.status === "used") {
        if (data.has_session) { location.replace(data.target || "/"); return; }
        fatal("该链接已使用过。请回群发送「#网页」（中台用「#中台」）重新获取登录链接。");
        return;
      }
      if (data.status === "expired") {
        fatal("链接已过期（约 10 分钟）。请回群里重新获取（#局面 / #网页 / #中台）。");
        return;
      }
      // pending：长轮询会挂起约 20 秒，这里直接重连。
    }
  }

  async function copyCmd() {
    var text = $("cmd").textContent;
    try {
      if (navigator.clipboard && navigator.clipboard.writeText) {
        await navigator.clipboard.writeText(text);
      } else {
        var ta = document.createElement("textarea");
        ta.value = text; ta.setAttribute("readonly", ""); ta.style.position = "fixed"; ta.style.opacity = "0";
        document.body.appendChild(ta); ta.select(); document.execCommand("copy"); document.body.removeChild(ta);
      }
      $("btn-copy").textContent = "已复制 ✓";
      setTimeout(function () { $("btn-copy").textContent = "一键复制"; }, 1800);
    } catch (e) {
      $("btn-copy").textContent = "复制失败，请手动长按复制";
    }
  }

  async function load() {
    var r = await api("/login/status?t=" + encodeURIComponent(TOKEN));
    if (!r.ok) { fatal(r.data.error || "链接已失效，请回群里重新获取。"); return; }
    state = r.data;
    $("who").hidden = false;
    $("who").innerHTML = "👤 <b></b>";
    $("who").querySelector("b").textContent = state.display_name || "（未获取到昵称）";
    $("info").hidden = false;
    var lines = ["群：" + (state.group_id ? "已绑定" : "-")];
    if (state.game && state.game.summary) lines.push(state.game.summary);
    if (state.mode === "reset") lines.push("本次操作：重置注册码");
    $("info").textContent = lines.join("\\n");
    $("info").style.whiteSpace = "pre-line";

    if (state.consumed) {
      if (state.has_session) { location.replace(state.target || "/"); return; }
      fatal("该链接已使用过。请回群发送「#网页」（中台用「#中台」）重新获取登录链接。");
      return;
    }
    if (state.locked_secs) {
      setErr("注册码错误次数过多，请 " + state.locked_secs + " 秒后再试。");
    }
    $("title").textContent = state.mode === "reset" ? "重置注册码" : "登录确认";
    if (state.stage === "code" && state.code) { renderCode(state.code); return; }
    if (state.stage === "confirmed") { show("step-confirmed"); poll(); return; }
    if (state.stage === "need_login") { show("step-login"); return; }
    show("step-register");
  }

  async function submit(kind) {
    var isRegister = kind === "register";
    var p1 = isRegister ? $("p1").value : $("p3").value;
    var p2 = isRegister ? $("p2").value : undefined;
    setErr("");
    var re = /^[0-9a-zA-Z]{6}$/;
    if (!re.test(p1)) { setErr("注册码需为 6 位数字或字母（0-9a-zA-Z）。"); return; }
    if (isRegister && p1 !== p2) { setErr("两次输入的注册码不一致。"); return; }
    var btn = isRegister ? $("btn-register") : $("btn-login");
    btn.disabled = true;
    var r = await api(isRegister ? "/login/register" : "/login/login", isRegister
      ? { t: TOKEN, passcode: p1, passcode_confirm: p2 }
      : { t: TOKEN, passcode: p1 });
    btn.disabled = false;
    if (!r.ok) { setErr(r.data.error || "操作失败，请重试。"); return; }
    renderCode(r.data.code);
  }

  $("btn-register").addEventListener("click", function () { submit("register"); });
  $("btn-login").addEventListener("click", function () { submit("login"); });
  $("btn-copy").addEventListener("click", copyCmd);
  $("btn-reset").addEventListener("click", function () {
    setErr("请回到 QQ 群，发送「#重置注册码」获取重置链接。");
  });
  ["p1","p2","p3"].forEach(function (id) {
    $(id).addEventListener("keydown", function (e) {
      if (e.key === "Enter") { submit(id === "p3" ? "login" : "register"); }
    });
  });
  load();
})();
</script>
</body>
</html>`;
}
