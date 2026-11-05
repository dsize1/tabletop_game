#!/usr/bin/env node
// P6.4 决策日志/导出/回放自检（自举：不依赖开发库历史）。
//
// 背景：规则修复后旧房间的 replay 必然不一致（属预期），只靠「开发库最近房间」会让门禁
// 在“规则刚改完但还没打新局”时误报。因此：
//   1) 优先在开发库最近房间中找一个**当前引擎可完整回放**的房间；
//   2) 若都不可用 → **自举**：临时 server + 临时 SQLite + MCP 现打一局（写入 replay_base/决策日志），
//      再回放该房间。这样门禁与开发库历史解耦，也避免手工 seed 房间。
//
// 校验内容（两种来源一致）：
//   a) `sgs-replay --json` mismatches=0；
//   b) `game_decision_log` 字段可解析（legal/candidates 为 JSON 数组，candidates ≤ 26）；
//   c) 导出 JSONL 行数与 `--limit` 一致、player_id 为哈希、字段稳定。
//
// 用法：npm run check:sgs-decision-log [--no-seed]
// 退出码：0 通过/跳过；1 失败。依赖：已构建 server / sgs-replay / game-engine-mcp；自举路径另需 Redis。

import { spawn, spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import fs from "node:fs/promises";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import { DatabaseSync } from "node:sqlite";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const args = new Set(process.argv.slice(2));
const allowSeed = !args.has("--no-seed");
const devDb = process.env.SQLITE_PATH ?? "data/tabletop.db";
const tag = "[check:sgs-decision-log]";
const children = [];

const log = (m) => console.log(`${tag} ${m}`);
const skip = (m) => {
  log(`跳过：${m}`);
  cleanup();
  process.exit(0);
};
function killTree(child) {
  try {
    if (process.platform === "win32" && child.pid) {
      spawnSync("taskkill", ["/F", "/T", "/PID", String(child.pid)], { stdio: "ignore" });
    } else {
      child.kill();
    }
  } catch {
    /* ignore */
  }
}
function cleanup() {
  for (const c of children) killTree(c);
}
const tcpOk = (host, port, timeoutMs = 700) =>
  new Promise((r) => {
    const s = net.connect({ host, port });
    const done = (ok) => {
      s.destroy();
      r(ok);
    };
    s.setTimeout(timeoutMs, () => done(false));
    s.once("connect", () => done(true));
    s.once("error", () => done(false));
  });
async function waitHealth(url, timeoutMs = 20000) {
  const end = Date.now() + timeoutMs;
  while (Date.now() < end) {
    try {
      const r = await fetch(url, { signal: AbortSignal.timeout(1200) });
      if (r.status < 500) return true;
    } catch {
      /* retry */
    }
    await new Promise((r) => setTimeout(r, 250));
  }
  return false;
}

const replayExe = path.join(
  root,
  "crates/target/debug",
  process.platform === "win32" ? "sgs-replay.exe" : "sgs-replay",
);
const serverExe = path.join(
  root,
  "crates/target/debug",
  process.platform === "win32" ? "server.exe" : "server",
);
const mcpExe = path.join(
  root,
  "crates/target/debug",
  process.platform === "win32" ? "game-engine-mcp.exe" : "game-engine-mcp",
);

function replayRoom(dbPath, room) {
  const argv = existsSync(replayExe)
    ? [replayExe, "--db", dbPath, "--room", room, "--json"]
    : [
        "cargo",
        "run",
        "--manifest-path",
        "crates/Cargo.toml",
        "-p",
        "server",
        "--bin",
        "sgs-replay",
        "--",
        "--db",
        dbPath,
        "--room",
        room,
        "--json",
      ];
  const rep = spawnSync(argv[0], argv.slice(1), { cwd: root, encoding: "utf8" });
  let mismatches = [];
  const jsonLine = (rep.stdout ?? "").trim().split("\n").filter(Boolean).pop();
  if (jsonLine) {
    try {
      const out = JSON.parse(jsonLine);
      if (Array.isArray(out.mismatches)) mismatches = out.mismatches;
    } catch {
      /* ignore */
    }
  }
  return { ok: rep.status === 0 && mismatches.length === 0, mismatches, stderr: rep.stderr ?? "" };
}

/** 字段 + 导出校验；返回失败描述数组。 */
function checkRows(dbPath, room) {
  const db = new DatabaseSync(dbPath);
  const fail = [];
  try {
    const rows = db
      .prepare(
        `select player_id, legal_actions, candidates, chosen from game_decision_log
         where room_id = ? order by seq, id limit 20`,
      )
      .all(room);
    if (rows.length === 0) fail.push("房间没有决策日志行");
    for (const r of rows) {
      try {
        const legal = JSON.parse(r.legal_actions);
        if (!Array.isArray(legal) || legal.length === 0) fail.push("legal_actions 不是非空数组");
      } catch {
        fail.push("legal_actions 不是合法 JSON");
      }
      if (r.candidates) {
        try {
          const c = JSON.parse(r.candidates);
          if (!Array.isArray(c)) fail.push("candidates 不是数组");
          if (c.length > 26) fail.push(`candidates 超过 26（${c.length}）`);
        } catch {
          fail.push("candidates 不是合法 JSON");
        }
      }
      if (r.chosen && typeof r.chosen !== "string") fail.push("chosen 类型异常");
    }
  } finally {
    db.close();
  }
  // 导出 5 条：哈希与字段。
  const exp = spawnSync(
    process.execPath,
    [resolve(root, "apps/scripts/sgs-export-decisions.mjs"), "--db", dbPath, "--room", room, "--limit", "5"],
    { encoding: "utf8" },
  );
  if (exp.status !== 0) {
    fail.push(`export 退出码 ${exp.status}: ${(exp.stderr ?? "").slice(0, 300)}`);
  } else {
    const lines = (exp.stdout ?? "").split("\n").filter((l) => l.trim().startsWith("{"));
    if (lines.length !== 5) fail.push(`导出行数 ${lines.length} ≠ limit 5`);
    for (const line of lines) {
      try {
        const o = JSON.parse(line);
        if (typeof o.player_hash !== "string" || !/^[0-9a-f]{16,}$/.test(o.player_hash)) {
          fail.push(`player_hash 不是哈希：${String(o.player_hash).slice(0, 24)}`);
        }
        if (!Array.isArray(o.legal)) fail.push("导出 legal 不是数组");
        if (!("room_id" in o) || !("seq" in o) || !("source" in o)) {
          fail.push("导出行缺少稳定字段（room_id/seq/source）");
        }
      } catch {
        fail.push("导出 JSONL 行不可解析");
      }
    }
  }
  return fail;
}

/** 在给定库上校验房间：回放 + 字段 + 导出。 */
function validate(dbPath, room) {
  const replay = replayRoom(dbPath, room);
  const problems = [...(replay.mismatches ?? [])];
  if (replay.stderr && !replay.ok && problems.length === 0) {
    problems.push(`replay stderr: ${replay.stderr.slice(-300)}`);
  }
  const rowProblems = checkRows(dbPath, room);
  return { ok: replay.ok && rowProblems.length === 0, problems: [...problems, ...rowProblems], replay };
}

/** 自举：临时 server + 临时 SQLite + MCP 现打一局，返回 {dbPath, room}。 */
async function seed() {
  if (!existsSync(serverExe) || !existsSync(mcpExe)) skip("server / game-engine-mcp 未构建（cargo build）");
  if (!(await tcpOk("127.0.0.1", 6379))) skip("Redis 不可达（server 启动依赖）");
  const tmpDir = await fs.mkdtemp(path.join(os.tmpdir(), "sgs-decision-"));
  const dbPath = path.join(tmpDir, "seed.db");
  const port = 3196;
  const token = "decision-e2e-token";
  const server = spawn(serverExe, [], {
    cwd: root,
    env: {
      ...process.env,
      SQLITE_PATH: dbPath,
      SERVER_PORT: String(port),
      REDIS_URL: process.env.REDIS_URL ?? "redis://127.0.0.1:6379",
      PROPS_ADMIN_TOKEN: token,
      DEV_WHITELIST: "decision-e2e",
      SGS_DECISION_LOG_RATE: "1",
      RUST_LOG: "warn",
    },
    stdio: ["ignore", "ignore", "pipe"],
  });
  children.push(server);
  server.stderr?.on("data", () => {});
  if (!(await waitHealth(`http://127.0.0.1:${port}/health`))) skip("临时 server 未就绪");

  const { Client } = await import("@modelcontextprotocol/sdk/client/index.js");
  const { StdioClientTransport } = await import("@modelcontextprotocol/sdk/client/stdio.js");
  const client = new Client({ name: "decision-e2e", version: "0.1.0" });
  await client.connect(
    new StdioClientTransport({
      command: mcpExe,
      args: [],
      cwd: root,
      env: { ...process.env, SQLITE_PATH: dbPath, SGS_DECISION_LOG_RATE: "1" },
    }),
  );
  const mcp = async (name, callArgs) => {
    const res = await client.callTool({ name, arguments: callArgs });
    const text = (res.content ?? []).find((c) => c.type === "text")?.text ?? "";
    if (text.startsWith("错误")) throw new Error(`${name}: ${text}`);
    return text ? JSON.parse(text) : null;
  };
  const players = ["seed-p1", "seed-p2", "seed-p3", "seed-p4", "seed-p5"];
  const created = await mcp("create_game", {
    game_id: "sanguosha",
    group_id: `decision-e2e-${Date.now()}`,
    host_id: players[0],
  });
  const room = created.room_id;
  for (const p of players.slice(1)) {
    await mcp("execute_action", { room_id: room, player_id: p, action: "lobby:join" });
  }
  await mcp("execute_action", { room_id: room, player_id: players[0], action: "lobby:start" });
  for (let i = 0; i < 80; i += 1) {
    let acted = false;
    for (const p of players) {
      const legal = await mcp("get_legal_actions", { room_id: room, player_id: p });
      const choose = Array.isArray(legal)
        ? legal.find((a) => a.id.startsWith("setup:choose_general:"))
        : null;
      if (choose) {
        await mcp("execute_action", { room_id: room, player_id: p, action: choose.id });
        acted = true;
        break;
      }
    }
    if (!acted) break;
  }
  for (let i = 0; i < 30; i += 1) {
    let moved = false;
    for (const p of players) {
      const legal = await mcp("get_legal_actions", { room_id: room, player_id: p });
      if (Array.isArray(legal) && legal.length) {
        const a = legal.find((x) => x.id === "pass") ?? legal[0];
        await mcp("execute_action", { room_id: room, player_id: p, action: a.id });
        moved = true;
        break;
      }
    }
    if (!moved) break;
  }
  await client.close();
  killTree(server);
  log(`自举房间：${room}（临时库 ${dbPath}）`);
  return { dbPath, room };
}

function devCandidates() {
  if (!existsSync(devDb)) return [];
  let db;
  try {
    db = new DatabaseSync(devDb);
    const table = db
      .prepare("select name from sqlite_master where type='table' and name='game_decision_log'")
      .get();
    if (!table) return [];
    return db
      .prepare(
        `select d.room_id, count(*) as n
         from game_decision_log d
         where exists (select 1 from game_replay_base b where b.room_id = d.room_id)
         group by d.room_id order by max(d.id) desc limit 8`,
      )
      .all();
  } catch {
    return [];
  } finally {
    db?.close();
  }
}

async function main() {
  for (const c of devCandidates()) {
    const r = validate(devDb, c.room_id);
    if (r.ok) {
      log(`通过（开发库）room=${c.room_id} logs=${c.n} mismatches=0`);
      return 0;
    }
  }
  if (!allowSeed) {
    console.error(`${tag} 失败：开发库没有可完整回放的房间（--no-seed 禁止自举）`);
    return 1;
  }
  log("开发库无当前引擎可回放房间 → 自举临时对局");
  const { dbPath, room } = await seed();
  const r = validate(dbPath, room);
  if (!r.ok) {
    console.error(`${tag} 失败：自举房间回放/字段校验不通过`);
    for (const p of r.problems.slice(0, 6)) console.error(`  ✗ ${p}`);
    return 1;
  }
  log(`通过（自举）room=${room} mismatches=0`);
  return 0;
}

try {
  const code = await main();
  cleanup();
  process.exit(code);
} catch (e) {
  cleanup();
  console.error(`${tag} ❌ ${e?.message ?? e}`);
  process.exit(1);
}
