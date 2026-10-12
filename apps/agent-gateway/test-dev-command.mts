// dev 命令解析层单测：固定前缀硬判断 + 需求/标题提取 + 能力格式化。
// 运行：npx tsx test-dev-command.mts
import assert from "node:assert";
import {
  classifyDevMessage,
  extractRequirement,
  extractSessionId,
  extractTitle,
  formatCapabilities,
  parseDevCommand,
} from "./src/dev-command.js";

// —— 命令解析（硬判断）——
assert.deepEqual(parseDevCommand("#任务"), { cmd: "任务", arg: undefined });
assert.deepEqual(parseDevCommand("#任务 running"), { cmd: "任务", arg: "running" });
assert.deepEqual(parseDevCommand("#进度  abc123  "), { cmd: "进度", arg: "abc123" });
assert.deepEqual(parseDevCommand("#确认 abc"), { cmd: "确认", arg: "abc" });
assert.deepEqual(parseDevCommand("#取消 abc"), { cmd: "取消", arg: "abc" });
assert.deepEqual(parseDevCommand("#能力"), { cmd: "能力", arg: undefined });
// 非命令不命中
assert.equal(parseDevCommand("#开发 做个功能"), null);
assert.equal(parseDevCommand("今天天气"), null);
assert.equal(parseDevCommand("#聊天"), null);

// —— 需求提取 ——
assert.equal(extractRequirement("#开发 帮我加个功能"), "帮我加个功能");
assert.equal(extractRequirement("#dev fix bug"), "fix bug");
assert.equal(extractRequirement("自然语言开发需求"), "自然语言开发需求");
assert.equal(extractRequirement("[开发] 会话id:b1314a38,查询任务状态"), "会话id:b1314a38,查询任务状态");

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
assert.equal(formatCapabilities("[]"), "无能力信息");
assert.equal(formatCapabilities("not-json"), "无能力信息");

// —— sessionId 提取 ——
assert.equal(
  extractSessionId("[开发] 会话id:b1314a38-f153-44a1-8f89-837fe5d26db3,查询任务状态"),
  "b1314a38-f153-44a1-8f89-837fe5d26db3",
);
assert.equal(extractSessionId("没有 uuid 的消息"), null);

// —— 语义分类（回归：查询不得误判为新建）——
assert.deepEqual(
  classifyDevMessage("[开发] 会话id:b1314a38-f153-44a1-8f89-837fe5d26db3,查询任务状态"),
  { type: "query", sessionId: "b1314a38-f153-44a1-8f89-837fe5d26db3" },
);
assert.deepEqual(
  classifyDevMessage("[开发]开发引擎mcp能力查询"),
  { type: "capabilities" },
);
assert.deepEqual(classifyDevMessage("#开发 帮我加个功能"), { type: "create" });
assert.deepEqual(classifyDevMessage("#开发"), { type: "create" });
assert.deepEqual(classifyDevMessage("帮我加个功能"), { type: "unknown" });
assert.deepEqual(classifyDevMessage("[开发]开发消息链路疑似异常"), { type: "unknown" });
assert.equal(extractRequirement("#开发"), "");
assert.equal(extractRequirement("#开发 做个功能"), "做个功能");
assert.deepEqual(
  classifyDevMessage("确认执行 b1314a38-f153-44a1-8f89-837fe5d26db3"),
  { type: "confirm", sessionId: "b1314a38-f153-44a1-8f89-837fe5d26db3" },
);
assert.deepEqual(classifyDevMessage("列出全部任务"), { type: "list" });

console.log("test-dev-command 全部通过 ✅");
