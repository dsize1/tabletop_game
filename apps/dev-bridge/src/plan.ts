//! 计划文档解析：统计 6 分区里「待办实现 / 验证事项」的复选框完成度。
//!
//! 与旧 `dev-task-mcp`（已删除）的 `count_plan_items` 语义一致
//! （契约测试会逐条对齐）：遇到 `## <section>` 切换分区，只统计该分区内的 `- [ ]` / `- [x]`。

import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));

/** 仓库根（apps/dev-bridge/src → ../../..）。 */
export function repoRoot(): string {
  return process.env.PROJECT_ROOT ?? resolve(__dirname, "../../..");
}

/** 相对路径按仓库根解析（`memory_palace/plan/{id}.md`）。 */
export function resolveInRepo(path: string): string {
  return path.startsWith("/") || /^[A-Za-z]:[\\/]/.test(path)
    ? path
    : resolve(repoRoot(), path);
}

export function countPlanItems(
  content: string,
  section: string,
): { done: number; total: number } {
  let inSection = false;
  let done = 0;
  let total = 0;
  for (const line of content.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (trimmed.startsWith("## ")) {
      inSection = trimmed.slice(3).trim() === section;
      continue;
    }
    if (!inSection || !trimmed.startsWith("- [")) continue;
    total += 1;
    if (trimmed.startsWith("- [x]") || trimmed.startsWith("- [X]")) done += 1;
  }
  return { done, total };
}

/** 读计划文档并统计两个分区；读不到时返回 0/0（与旧实现一致）。 */
export function readPlanProgress(planPath: string | null): {
  todo: { done: number; total: number };
  verify: { done: number; total: number };
} {
  if (!planPath) return { todo: { done: 0, total: 0 }, verify: { done: 0, total: 0 } };
  let content: string;
  try {
    content = readFileSync(resolveInRepo(planPath), "utf8");
  } catch {
    return { todo: { done: 0, total: 0 }, verify: { done: 0, total: 0 } };
  }
  return {
    todo: countPlanItems(content, "待办实现"),
    verify: countPlanItems(content, "验证事项"),
  };
}
