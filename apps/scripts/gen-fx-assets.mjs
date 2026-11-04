#!/usr/bin/env node
// 三国杀对局特效素材：ComfyUI 生成 + 本地打包（P0/P1）。
//
// 用法：
//   node apps/scripts/gen-fx-assets.mjs --dry-run
//   node apps/scripts/gen-fx-assets.mjs            # 调 ComfyUI 出图 → assemble
//   node apps/scripts/gen-fx-assets.mjs --only slash,fire
//   node apps/scripts/gen-fx-assets.mjs --assemble # 不调 ComfyUI：用户改完源图后重打包
//
// 源图：assets-src/fx/src/<id>.png（本地，gitignore）
// 产物：apps/web/public/fx/<id>-<hash>.webp + apps/web/src/view/sgs/fx-manifest.ts
//
// 说明：ComfyUI 出“纯黑底 VFX”，这里按亮度做 alpha 键控 + 裁掉空白；
// 运行时用 mix-blend-mode: screen 叠加，黑底天然透明。

import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("../..", import.meta.url));
const SRC_DIR = path.join(ROOT, "assets-src", "fx", "src");
const OUT_DIR = path.join(ROOT, "apps", "web", "public", "fx");
const MANIFEST = path.join(ROOT, "apps", "web", "src", "view", "sgs", "fx-manifest.ts");
const PACK_SCRIPT = path.join(ROOT, "apps", "scripts", "fx_pack.py");
const PYTHON =
  process.env.PYTHON_BIN ??
  (existsSync("C:/llm/comfy-venv/Scripts/python.exe")
    ? "C:/llm/comfy-venv/Scripts/python.exe"
    : "python");
const PROMPTS_FILE = path.join(ROOT, "assets-src", "fx", "prompts.json");
const PROMPTS = JSON.parse(await fs.readFile(PROMPTS_FILE, "utf8"));
const COMFY = (process.env.COMFY_URL ?? "http://127.0.0.1:8188").replace(/\/+$/, "");
const CKPT = process.env.FX_CKPT ?? PROMPTS.checkpoint ?? "animagine-xl-3.1.safetensors";
const STEPS = 28;
const CFG = 6.5;
const SIZE = 1024;
const ANIM_SIZE = 168;
const QUALITY = 36;

const args = process.argv.slice(2);
const has = (f) => args.includes(f);
const val = (f, d = null) => {
  const i = args.indexOf(f);
  return i >= 0 && args[i + 1] ? args[i + 1] : d;
};
const DRY = has("--dry-run");
const ASSEMBLE_ONLY = has("--assemble");
const ONLY = (val("--only", "") ?? "").split(",").map((s) => s.trim()).filter(Boolean);

const NEG = PROMPTS.negative;

const EFFECTS = PROMPTS.effects;

function log(...a) {
  console.log("[fx-assets]", ...a);
}

function selectEffects() {
  if (ONLY.length === 0) return EFFECTS;
  const map = new Map(EFFECTS.map((e) => [e.id, e]));
  return ONLY.map((id) => map.get(id)).filter(Boolean);
}

async function comfyJson(pathname, init) {
  const r = await fetch(`${COMFY}${pathname}`, init);
  if (!r.ok) throw new Error(`ComfyUI ${pathname} ${r.status}`);
  return r.json();
}

async function waitComfy() {
  for (let i = 0; i < 120; i += 1) {
    try {
      await comfyJson("/system_stats");
      return;
    } catch {
      await new Promise((r) => setTimeout(r, 1000));
    }
  }
  throw new Error(`ComfyUI 未就绪：${COMFY}`);
}

function workflow(effect, seed) {
  const pos = `${effect.prompt}, ${PROMPTS.suffix}`;
  return {
    1: { class_type: "CheckpointLoaderSimple", inputs: { ckpt_name: CKPT } },
    2: {
      class_type: "CLIPTextEncode",
      inputs: { text: pos, clip: ["1", 1] },
    },
    3: {
      class_type: "CLIPTextEncode",
      inputs: { text: NEG, clip: ["1", 1] },
    },
    4: {
      class_type: "EmptyLatentImage",
      inputs: { width: SIZE, height: SIZE, batch_size: 1 },
    },
    5: {
      class_type: "KSampler",
      inputs: {
        seed,
        steps: STEPS,
        cfg: CFG,
        sampler_name: "dpmpp_2m",
        scheduler: "karras",
        denoise: 1,
        model: ["1", 0],
        positive: ["2", 0],
        negative: ["3", 0],
        latent_image: ["4", 0],
      },
    },
    6: { class_type: "VAEDecode", inputs: { samples: ["5", 0], vae: ["1", 2] } },
    7: {
      class_type: "SaveImage",
      inputs: { filename_prefix: `fx/${effect.id}`, images: ["6", 0] },
    },
  };
}

async function genOne(effect) {
  const seed = 20261104 + EFFECTS.findIndex((e) => e.id === effect.id) * 131;
  const outPath = path.join(SRC_DIR, `${effect.id}.png`);
  if (existsSync(outPath) && !has("--force")) {
    log(`skip（已有源图）：${effect.id}`);
    return;
  }
  const res = await comfyJson("/prompt", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ prompt: workflow(effect, seed), client_id: "sgs-fx" }),
  });
  const promptId = res.prompt_id;
  if (!promptId) throw new Error(`提交失败：${JSON.stringify(res)}`);
  log(`生成 ${effect.id} … prompt=${promptId}`);
  let images = null;
  for (let i = 0; i < 300; i += 1) {
    const h = await comfyJson(`/history/${promptId}`);
    const entry = h[promptId];
    if (entry?.status?.status_str === "error") {
      throw new Error(`ComfyUI 执行失败：${effect.id}`);
    }
    images = entry?.outputs?.["7"]?.images;
    if (images?.length) break;
    await new Promise((r) => setTimeout(r, 700));
  }
  if (!images?.length) throw new Error(`超时无输出：${effect.id}`);
  const img = images[0];
  const q = new URLSearchParams({
    filename: img.filename,
    subfolder: img.subfolder ?? "fx",
    type: img.type ?? "output",
  });
  const r = await fetch(`${COMFY}/view?${q}`);
  if (!r.ok) throw new Error(`下载失败 ${effect.id}: ${r.status}`);
  const buf = Buffer.from(await r.arrayBuffer());
  await fs.mkdir(SRC_DIR, { recursive: true });
  await fs.writeFile(outPath, buf);
  log(`done ${effect.id} → ${path.relative(ROOT, outPath)}`);
}

async function assemble(effects) {
  await fs.mkdir(OUT_DIR, { recursive: true });
  const meta = [];
  for (const effect of effects) {
    const src = path.join(SRC_DIR, `${effect.id}.png`);
    if (!existsSync(src)) {
      log(`缺源图，跳过：${effect.id}（先 npm run gen:fx）`);
      continue;
    }
    const tmpOut = path.join(ROOT, ".tmp", `fx-pack-${effect.id}.webp`);
    const packed = spawnSync(
      PYTHON,
      [PACK_SCRIPT, src, tmpOut, "--kind", effect.id, "--size", String(ANIM_SIZE), "--quality", String(QUALITY), "--frames", "5", "--duration", "80"],
      { encoding: "utf8" },
    );
    if (packed.status !== 0) {
      throw new Error(`fx_pack 失败（${effect.id}）：${packed.stderr || packed.stdout}`);
    }
    const anim = JSON.parse((packed.stdout || "").trim().split("\n").pop() || "{}");
    const buf = await fs.readFile(tmpOut);
    await fs.unlink(tmpOut).catch(() => undefined);
    const hash = createHash("sha1").update(buf).digest("hex").slice(0, 8);
    const file = `${effect.id}-${hash}.webp`;
    await fs.writeFile(path.join(OUT_DIR, file), buf);
    meta.push({
      id: effect.id,
      url: `/fx-assets/${file}`,
      width: anim.width ?? ANIM_SIZE,
      height: anim.height ?? ANIM_SIZE,
      frames: anim.frames ?? 5,
      duration: anim.duration ?? 80,
      blend: "normal",
      bytes: buf.length,
    });
    log(`assemble ${effect.id} → ${file} (${Math.round(buf.length / 1024)}KB, ${anim.width}×${anim.height}, ${anim.frames} 帧)`);
  }

  // 清理不再引用的旧产物
  const keep = new Set(meta.map((m) => path.basename(m.url)));
  for (const f of await fs.readdir(OUT_DIR).catch(() => [])) {
    if (/\.webp$/.test(f) && !keep.has(f)) {
      await fs.unlink(path.join(OUT_DIR, f)).catch(() => undefined);
    }
  }

  const lines = [
    "// AUTO-GENERATED by apps/scripts/gen-fx-assets.mjs --assemble（勿手改；改素材后重跑）。",
    "",
    "export interface FxAssetMeta {",
    '  url: string;',
    '  width: number;',
    '  height: number;',
    '  frames: number;',
    '  duration: number;',
    '  blend: "screen" | "normal";',
    "}",
    "",
    "export const FX_ASSETS: Record<string, FxAssetMeta> = {",
    ...meta.map(
      (m) =>
        `  ${JSON.stringify(m.id)}: { url: ${JSON.stringify(m.url)}, width: ${m.width}, height: ${m.height}, frames: ${m.frames}, duration: ${m.duration}, blend: ${JSON.stringify(m.blend)} },`,
    ),
    "};",
    "",
  ];
  await fs.writeFile(MANIFEST, lines.join("\n"), "utf8");
  const total = meta.reduce((n, m) => n + m.bytes, 0);
  log(`manifest → ${path.relative(ROOT, MANIFEST)}；${meta.length} 个效果，总计 ${Math.round(total / 1024)}KB`);
}

async function main() {
  const effects = selectEffects();
  if (effects.length === 0) throw new Error("没有匹配的效果 id");
  if (DRY) {
    for (const e of effects) log(`dry-run: ${e.id} (${e.blend}) → assets-src/fx/src/${e.id}.png`);
    return;
  }
  if (!ASSEMBLE_ONLY) {
    await waitComfy();
    for (const e of effects) await genOne(e);
  }
  // 始终按全量重打包 manifest/清理：`--only` 只限制“生成哪几张”，不能把其他效果从清单里删掉。
  await assemble(EFFECTS);
}

main().catch((e) => {
  console.error(`[fx-assets] ❌ ${e.message}`);
  process.exit(1);
});
