// docs-mcp 命令与链接拼接单测。
// 运行：npx tsx test-docs.mts
import assert from "node:assert";
import { parseDevCommand } from "./src/dev-command.js";
import { buildDocsUrl } from "./src/auth-client.js";

// #文档：无参数命令，覆盖全部根
assert.deepEqual(parseDevCommand("#文档"), { cmd: "文档", arg: undefined });
assert.equal(parseDevCommand("#文档").cmd, "文档");
// 非命令不命中
assert.equal(parseDevCommand("查看文档"), null);
assert.equal(parseDevCommand("#文件"), null);

// 文档短链拼接（去尾部斜杠）
assert.equal(buildDocsUrl("https://x.test/", "tok"), "https://x.test/dev_docs?token=tok");
assert.equal(buildDocsUrl("https://x.test", "tok"), "https://x.test/dev_docs?token=tok");

console.log("test-docs 全部通过 ✅");
