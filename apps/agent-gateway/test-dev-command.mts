// dev 命令解析层单测：显式前缀命令 + 需求/引用/标题提取 + 能力格式化。
// 运行：npx tsx test-dev-command.mts
// 整改（2026-10-13）：dev 只认显式命令，禁止对自由文本做意图猜测。
import assert from "node:assert";
import {
  extractRequirement,
  extractSessionId,
  extractTitle,
  formatCapabilities,
  parseDevCommand,
  splitTaskRef,
} from "./src/dev-command.js";

// —— 命令解析（显式前缀，非意图猜测）——
assert.deepEqual(parseDevCommand("#任务"), { cmd: "任务", arg: undefined });
assert.deepEqual(parseDevCommand("#任务 running"), { cmd: "任务", arg: "running" });
assert.deepEqual(parseDevCommand("#进度  abc123  "), { cmd: "进度", arg: "abc123" });
assert.deepEqual(parseDevCommand("#确认 abc"), { cmd: "确认", arg: "abc" });
assert.deepEqual(parseDevCommand("#取消 abc"), { cmd: "取消", arg: "abc" });
assert.deepEqual(parseDevCommand("#能力"), { cmd: "能力", arg: undefined });
assert.deepEqual(parseDevCommand("#发起 做个功能"), { cmd: "发起", arg: "做个功能" });
assert.deepEqual(parseDevCommand("#开发 做个功能"), { cmd: "发起", arg: "做个功能" }); // 历史别名
assert.deepEqual(parseDevCommand("#补充 abc 追加内容"), { cmd: "补充", arg: "abc 追加内容" });

// 非命令不命中
assert.equal(parseDevCommand("今天天气"), null);
assert.equal(parseDevCommand("#聊天"), null);
// 回归：这类自由文本不得被判为命令（历史误判「不要解除围栏开始执行」→ confirm 的根因）
assert.equal(parseDevCommand("不要解除围栏开始执行"), null);
assert.equal(parseDevCommand("#编辑 任务：abc 不要解除围栏开始执行"), null);

// —— 需求提取 ——
assert.equal(extractRequirement("#发起 帮我加个功能"), "帮我加个功能");
assert.equal(extractRequirement("#开发 帮我加个功能"), "帮我加个功能");
assert.equal(extractRequirement("#dev fix bug"), "fix bug");
assert.equal(extractRequirement("[开发] 会话id:b1314a38,查询任务状态"), "会话id:b1314a38,查询任务状态");
assert.equal(extractRequirement("#发起"), "");

// —— #补充 引用拆分（支持 <uuid> 与「任务：<uuid>，」两种写法、正文可多行）——
const r1 = splitTaskRef("b1314a38-f153-44a1-8f89-837fe5d26db3 待确定事项裁定如下：平局=顺延");
assert.equal(r1.sessionId, "b1314a38-f153-44a1-8f89-837fe5d26db3");
assert.equal(r1.content, "待确定事项裁定如下：平局=顺延");
const r2 = splitTaskRef("任务：b1314a38-f153-44a1-8f89-837fe5d26db3，规则参考如下：\n一、胜负条件");
assert.equal(r2.sessionId, "b1314a38-f153-44a1-8f89-837fe5d26db3");
assert.equal(r2.content, "规则参考如下：\n一、胜负条件");
assert.equal(splitTaskRef("没有 uuid").sessionId, null);

// —— 标题提取 ——
assert.equal(extractTitle("标题：新增登录模块\n\n计划...", "兜底"), "新增登录模块");
assert.equal(extractTitle("没有标题行的输出", "需求标题"), "需求标题");

// —— 能力格式化 ——
const capsJson = JSON.stringify([
  { name: "list_tasks", rw: "读", description: "列出任务", usage: "list_tasks()" },
  { name: "create_task", rw: "写", description: "创建任务", usage: "create_task(id,g,q)" },
]);
assert.ok(formatCapabilities(capsJson).includes("list_tasks [读] 列出任务"));
assert.ok(formatCapabilities(capsJson).includes("create_task [写] 创建任务"));
// 含 example 时应渲染「示例：」行
const capsWithExample = JSON.stringify([
  { name: "list_tasks", rw: "读", description: "列出任务", usage: "list_tasks()", example: "#任务" },
]);
assert.ok(formatCapabilities(capsWithExample).includes("示例：#任务"));
assert.equal(formatCapabilities("[]"), "无能力信息");
assert.equal(formatCapabilities("not-json"), "无能力信息");

// —— sessionId 提取 ——
assert.equal(
  extractSessionId("[开发] 会话id:b1314a38-f153-44a1-8f89-837fe5d26db3,查询任务状态"),
  "b1314a38-f153-44a1-8f89-837fe5d26db3",
);
assert.equal(extractSessionId("没有 uuid 的消息"), null);
// 命令参数带尾部文本时仍能提取 UUID（回归：#确认 <id>」开始执行 场景）
assert.equal(
  extractSessionId("cc842d21-0c4f-48e1-8849-8b06fd367ed6」开始执行"),
  "cc842d21-0c4f-48e1-8849-8b06fd367ed6",
);

console.log("test-dev-command 全部通过 ✅");
