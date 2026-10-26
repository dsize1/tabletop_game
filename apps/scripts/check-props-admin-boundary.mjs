#!/usr/bin/env node
// 安全边界：道具中台在 P1 不允许挂到公网 web/tunnel。
//
// 校验：
// - apps/web（公网 web）不得挂载 /props-admin 或代理 /internal/gameprops；
// - server 必须只监听 127.0.0.1；
// - 文档存在「仅本机 / 不挂公网」说明（宽松检查）。
//
// 用法：node apps/scripts/check-props-admin-boundary.mjs

import { readFileSync, existsSync } from "node:fs";

const WEB_SRC = "apps/web/src/index.ts";
const SERVER_MAIN = "crates/server/src/main.rs";
const DOC = "docs/modules/props-admin.md";

function fail(msg) {
  console.error(`❌ ${msg}`);
  process.exitCode = 1;
}

function read(path) {
  if (!existsSync(path)) {
    fail(`缺少文件 ${path}`);
    return "";
  }
  return readFileSync(path, "utf8");
}

const web = read(WEB_SRC);
for (const needle of ["/props-admin", "/internal/gameprops"]) {
  if (web.includes(needle)) {
    fail(`公网 web 源码出现 ${needle}；P1 中台不得挂公网（P8 权鉴后再开放）`);
  }
}

const server = read(SERVER_MAIN);
if (!server.includes("127.0.0.1")) {
  fail("server 未找到 127.0.0.1 监听约束");
}
if (/SocketAddr::from\(\(\[0, 0, 0, 0\]/.test(server)) {
  fail("server 监听 0.0.0.0；道具中台 API 可能被公网访问");
}

const doc = read(DOC);
if (!/不挂公网|仅本机/.test(doc)) {
  fail("props-admin 文档缺少「仅本机 / 不挂公网」边界说明");
}

if (process.exitCode) {
  console.error("道具中台公网边界检查未通过");
} else {
  console.log("✅ 道具中台公网边界检查通过（仅本机，不挂公网）");
}
