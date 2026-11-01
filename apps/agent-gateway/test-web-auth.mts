// Web 跨端登录命令解析与链接拼装单测（ADR-0031）。
// 运行：npx tsx test-web-auth.mts
import assert from "node:assert";
import {
  buildLoginUrl,
  formatLinkLine,
  parseAuthCommand,
} from "./src/web-auth.js";

// —— #验证码#NNNN（含空格 / 全角井号 / 兼容写法） ——
assert.deepEqual(parseAuthCommand("#验证码#1234"), { cmd: "verify", code: "1234" });
assert.deepEqual(parseAuthCommand("#验证码 0000"), { cmd: "verify", code: "0000" });
assert.deepEqual(parseAuthCommand("  #验证码＃0042  "), { cmd: "verify", code: "0042" });

// —— 非法验证码：不落 auth（交回其它路由/闲聊） ——
for (const bad of ["#验证码#123", "#验证码#12345", "#验证码#12a4", "#验证码", "#验证码#"]) {
  assert.equal(parseAuthCommand(bad), null, `${bad} 不应被解析`);
}

// —— 其它命令 ——
assert.deepEqual(parseAuthCommand("#网页"), { cmd: "web" });
assert.deepEqual(parseAuthCommand("#我的链接"), { cmd: "web" });
assert.deepEqual(parseAuthCommand("#登录"), { cmd: "web" });
assert.deepEqual(parseAuthCommand("#中台"), { cmd: "props" });
assert.deepEqual(parseAuthCommand("#重置注册码"), { cmd: "reset" });
assert.deepEqual(parseAuthCommand("#重置密码"), { cmd: "reset" });

// —— 非登录命令不受影响 ——
for (const other of ["#开局", "#加入", "#局面", "#验证", "#中台 看看", "验证码#1234"]) {
  assert.equal(parseAuthCommand(other), null, `${other} 不应被当成登录命令`);
}

// —— 登录链接：与游戏页同源，token 只出现在 t 参数 ——
const url = buildLoginUrl("https://tabletop.example.com/", "abc-123");
assert.equal(url, "https://tabletop.example.com/login?t=abc-123");
assert.ok(!url.includes("token="), "登录链接不得携带正式 token");
assert.ok(buildLoginUrl("http://127.0.0.1:18080", "x y").includes("t=x%20y"));

// —— 链接列表行格式 ——
assert.equal(formatLinkLine("甲", "https://x/1"), "· 甲：https://x/1");

console.log("✅ test-web-auth 通过");
