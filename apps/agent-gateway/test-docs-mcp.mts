// docs-mcp 集成验证（需先 cargo build -p docs-mcp 产出二进制）。
// 运行：npx tsx test-docs-mcp.mts
// 可用 DOCS_MCP 指定二进制路径（默认 crates/target/debug）。
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";
import path from "node:path";
import { fileURLToPath } from "node:url";
import assert from "node:assert";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const exe =
  process.env.DOCS_MCP ??
  path.resolve(
    __dirname,
    "../../crates/target/debug",
    process.platform === "win32" ? "docs-mcp.exe" : "docs-mcp",
  );
const projectRoot = path.resolve(__dirname, "../..");

const transport = new StdioClientTransport({
  command: exe,
  args: [],
  cwd: projectRoot,
  env: {
    DEV_WHITELIST: "testdev",
    DOCS_CONFIG: "config/dev-docs.json",
  },
});
const client = new Client({ name: "smoke", version: "0.1.0" });
await client.connect(transport);

async function call(name: string, args: Record<string, unknown> = {}): Promise<string> {
  const result = await client.callTool({ name, arguments: args });
  const content = result.content as Array<{ type: string; text?: string }>;
  return content.find((c) => c.type === "text")?.text ?? "";
}

// 能力清单：应含 prepare_docs_link + list_capabilities，且带 usage/example
const caps = JSON.parse(await call("list_capabilities"));
assert.ok(Array.isArray(caps) && caps.length >= 2, "能力清单应含全部工具");
const link = caps.find((c: { name: string }) => c.name === "prepare_docs_link");
assert.ok(link, "能力清单应含 prepare_docs_link");
assert.equal(link.rw, "读");
assert.ok(link.usage && link.example, "能力描述符应含 usage 与 example");

// 白名单：返回 docs:all 与全部根（4 个）
const allowed = JSON.parse(await call("prepare_docs_link", { qq_id: "testdev" }));
assert.equal(allowed.resource, "docs:all");
assert.deepEqual(allowed.roots, ["docs", "memory_palace", "prompts", "skills"]);

// 非白名单被拒
const denied = await call("prepare_docs_link", { qq_id: "outsider" });
assert.ok(denied.includes("错误"), "非白名单应被拒");

console.log("test-docs-mcp 全部通过 ✅");
await transport.close();
