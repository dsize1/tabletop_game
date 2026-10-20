// dev 契约快照测试：工具名/描述、能力描述符必须与 fixtures/dev-contract.json 完全一致。
// 目的：任何「改工具但忘了改契约/文档」都会在这里失败（#能力 与网关调用点依赖它）。
// 升级方式：确认变更是有意的 → 重新生成 fixture（见文件底部说明）。
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { TOOLS } from "../dev-bridge/src/main.ts";
import { CAPABILITIES } from "../dev-bridge/src/primitives.ts";

const here = dirname(fileURLToPath(import.meta.url));
const fixturePath = join(here, "fixtures", "dev-contract.json");
const fixture = JSON.parse(readFileSync(fixturePath, "utf8")) as {
  tools: Array<{ name: string; description: string }>;
  capabilities: Array<Record<string, string>>;
};

const currentTools = TOOLS.map((t) => ({ name: t.name, description: t.description }));
assert.deepEqual(
  currentTools.map((t) => t.name),
  fixture.tools.map((t) => t.name),
  "工具名/顺序变更：如属有意，请同步 fixture + 文档 + 网关调用点",
);
assert.deepEqual(currentTools, fixture.tools, "工具描述变更：请同步 fixture（#能力/文档依赖它）");
assert.deepEqual(CAPABILITIES, fixture.capabilities, "能力描述符变更：请同步 fixture（用户可见契约）");

// 数量守卫（与 fixture 独立，防止两边一起漂移）
assert.equal(TOOLS.length, 19, "工具数应为 19（11 旧 + 8 v2）");
assert.equal(CAPABILITIES.length, 11, "能力描述符应保持 11 项（#能力 渲染）");

console.log("OK test-dev-contract 通过（19 tools / 11 capabilities 与快照一致）");

// 重新生成（在 apps/agent-gateway 下）：
//   npx tsx -e 不行（tsx 不支持 -e）→ 临时 .mts 脚本 import TOOLS/CAPABILITIES 后写 fixtures/dev-contract.json
