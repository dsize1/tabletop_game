#!/usr/bin/env node
// 全武将验证 · 批次走查单生成器（人工签字用）。
//
// 边界（memory_palace/plan/next-general-audit.md §4.0）：
// - 只汇总**已有事实**：`skills.md` 原文、目录技能类型、`data/audit/<batch>.json` 机检结果；
// - **不生成 `expected`/结论**，不写矩阵人工列（结论/确认人/日期留空，由人工填写）；
// - 场景构造提示只由原文条件反推（如「手牌多于你」「成为【杀】目标后」），不是语义裁定。
//
// 用法：
//   node apps/scripts/sgs-audit-sheet.mjs --batch B0
//   node apps/scripts/sgs-audit-sheet.mjs --batch B1 --batch B2   # 多个批次
//   node apps/scripts/sgs-audit-sheet.mjs --all
//
// 输出：docs/games/sanguosha/audit-sheets/<BATCH>.md（生成物，人工结论请写回矩阵）

import fs from "node:fs/promises";
import path from "node:path";
import { loadCatalog, projectRoot } from "./lib/sgs-catalog.mjs";
import { expandBatches } from "./lib/sgs-batches.mjs";

const args = process.argv.slice(2);
const flag = (n) => args.includes(n);
const opts = (n) => {
  const out = [];
  args.forEach((a, i) => {
    if (a === n) {
      const v = args[i + 1];
      if (v && !v.startsWith("--")) out.push(v);
    }
  });
  return out;
};
const opt = (n, d) => opts(n).at(-1) ?? d;

const SKILLS_MD = path.join(projectRoot, "docs/games/sanguosha/skills.md");
const AUDIT_DIR = path.join(projectRoot, "data/audit");
const OUT_DIR = path.join(projectRoot, "docs/games/sanguosha/audit-sheets");
const MATRIX_REL = "docs/games/sanguosha/general-audit.md";

/** skills.md 类型列（中文）→ 目录 kind。 */
const KIND_ZH = {
  normal: "普通",
  locked: "锁定",
  lord: "主公",
  limited: "限定",
  awaken: "觉醒",
  transform: "转换",
  trigger: "触发",
};

/** kind → 通用走查判定点（不含语义结论）。 */
const KIND_CHECKS = {
  normal: [
    "入口：合法时出现（`skill:<code>` 或转化技动作），非法时置灰/不出现",
    "选择：目标/牌数/颜色花色限制与原文一致；可取消且取消后无副作用",
    "边界：无手牌/无装备/无合法目标时的表现（不报错、不静默吞掉）",
    "结算：区域变化、体力/标记、后续 pending 链与日志一致",
  ],
  locked: [
    "自动生效、不出现「发动/取消」按钮（或按钮不可点）",
    "与装备/身份/横置/翻面交互符合原文",
    "边界：条件不满足时确实不生效（不误触发）",
  ],
  lord: [
    "主公身份：技能出现且可响应；非主公身份：不出现（或按原文限定）",
    "同势力/异势力角色响应与拒绝分支各走一遍",
    "响应链与【杀】/【闪】/【桃】时机、次数限制一致",
  ],
  limited: [
    "每局一次：发动后本局不再出现（含读档/回放）",
    "时机与条件（原文）在该时机之外不出现",
  ],
  awaken: [
    "条件未满足时不出现；满足后弹出觉醒结算",
    "觉醒后：体力上限/技能替换/新技能入口正确",
  ],
  transform: [
    "阴阳两面切换入口与当前面展示正确",
    "阳/阴分支各自的时机与限制（每回合/每阶段次数）",
    "颜色/花色/距离/次数限制与原文一致",
  ],
  trigger: [
    "触发时机（准备/判定/摸牌/出牌/弃牌/结束/回合外）正确",
    "是否可选（可拒绝）与拒绝后的状态",
    "一次结算只触发一次（不重复入栈）",
  ],
};

/** 开局技：sandbox 开局时已压 pending，预览 scene 注入会丢弃 → 必须用 `--no-scene` / 沙盒直连。 */
const OPENING_SKILLS = new Set(["mingren", "huaiju", "zongzuo", "xueyi", "qiaobian", "huashen"]);

/** 已知「L1 未触达」原因 → L2 场景构造提示（只描述构造条件，不判语义）。 */
const SCENE_HINTS = {
  jushou: "结束阶段：场景设 turn_phase=end（未翻面）；「弃 1 张手牌」与「装备牌改为使用」两条分支各走一遍",
  jiewei: "①需装备区有牌（`skill:jiewei:<card>` 当无懈）②需先翻面（flipped）再触发翻回（放逐/酒诗/举荐/回合开始跳过）→ 三阶段 msrc/move/to",
  buqu: "濒死：体力调至 0 + 构造两种判定（点数互不相同 → 回复 1；重复 → 新牌进弃牌堆）；再核对有「创」时手牌上限=创数",
  fenji: "回合外（他人回合）+ 手牌被弃置（过河/寒冰剑/顺手）或手牌数增加（五谷/回合外遗计）→ 询问摸 2；自己回合内弃牌不应触发",
  xingshang: "其他角色死亡（非终局）：死亡清理前询问获得其所有牌",
  fangzhu: "受伤后：令一名其他角色翻面并摸 X（X=已损失体力）",
  songwei: "主公技：需魏势力其他角色的黑色判定生效 → 自动令曹丕摸 1",
  // —— B1 ——
  yongsi: "锁定：摸牌阶段多摸 X、弃牌阶段开始先弃 X（X=存活角色势力数）→ 数势力数与摸/弃数量",
  qizhi: "回合内使用非装备牌（基本/锦囊）后 → 弃一名其他角色一张牌，然后其摸 1；核对 qizhi_count 累计",
  jinzou: "结束阶段：摸 2 后弃至 X（X=本回合奇制次数；X=0 弃光）→ 先做 0 次与 N 次奇制两种",
  kuizhu: "弃牌阶段结束且本阶段有弃牌 → 二选一（令至多 X 名各摸 1 / 对体力之和为 X 的角色各 1 伤）",
  chezheng: "出牌阶段对「攻击范围不含你」的角色造成伤害 → 伤害被防止；出牌阶段结束时用牌数 < 这些角色数 → 弃其中一名一张牌",
  jianxiang: "成为其他角色使用牌的目标后 → 令手牌最少的一名角色摸 1（并列随机）",
  shenshi: "转换技：阳=出牌阶段限一次交给手牌最多者一张牌并造成 1 伤；阴=其他角色对你造成伤害后观看其手牌并交牌，其结束阶段未失去则你摸至 4",
  chenglue: "转换技：阳=摸 1 弃 2；阴=摸 2 弃 1；然后本阶段与弃置牌同花色的牌无距离与次数限制",
  shicai: "本回合首次使用基本牌/锦囊后 → 置于牌堆顶然后摸 1（装备不触发）",
  cunmu: "锁定：所有摸牌改为从牌堆底摸 → 用牌堆顶/底不同牌区分验证",
  juzhan: "转换技：需「其他角色对你使用【杀】」（阳）/「你使用【杀】指定目标后」（阴）→ 场景给其他座位【杀】并令其对目标出杀（`--no-scene` 机检已触达）",
  feijun: "需存在「手牌多于你」或「装备区多于你」的角色 → 场景把其他座位手牌调到 > 目标手牌",
  binglue: "随飞军触发（首次对某角色发动）→ 与 feijun 同场景，注意 flags[\"feijun:<seat>\"]",
  mingren: "开局技：沙盒开局会压 `choose:mingren`（实测 `--no-scene` 可触达）→ 预览注入 scene 会丢弃该 pending；再走查结束阶段替换「任」",
  zhenliang: "需「任」已有牌且手牌有同色牌 → 场景先构造「任」标记与同色牌",
  zhengrong: "需使用【杀】/伤害锦囊且目标手牌数 ≥ 你 → 场景给目标更多手牌",
  hongju: "觉醒：需「荣」≥3 → 场景预置 3 枚「荣」再走准备阶段",
  huaiju: "开局获得 3 枚「橘」→ 确认开局钩子与「橘」标记展示",
  zhenglun: "需「无橘」→ 场景先移除全部「橘」标记，再走摸牌阶段",
  lijun: "主公技：需吴势力角色在其出牌阶段使用【杀】后交给你 → 场景指定吴势力友军",
  weidi: "锁定技：视为拥有主公已实现的主公技 → 依赖主公武将与势力",
  // —— B2 ——
  zhengu: "结束阶段选一名其他角色 → 本回合结束时与其下回合结束时，其手牌调整至与你相同（至多 5，超出弃置）",
  wanglie: "出牌阶段首张牌无距离限制；使用【杀】/单目标普通锦囊（无懈除外）时可令其不能被响应 → 然后本阶段不能再使用牌",
  zuilun: "结束阶段观看牌堆顶 3 张 → 按三项条件（造成过伤害/未弃置过牌/手牌最少）各取一张，剩余按原序回置；全不满足则与一名其他角色各失 1 体力",
  fuyin: "锁定：每回合首次成为其他角色【杀】/【决斗】目标后，若其手牌数 ≥ 你 → 此牌对你无效",
  liangyin: "每回合首次有牌移出/移入游戏后 → 与一名其他角色各摸 1 或各弃 1；然后可令手牌数=「箜声」牌数的角色回复 1",
  kongsheng: "准备阶段置任意张牌于武将牌上；结束阶段获得其中非装备牌，再令一名角色使用剩余装备牌并失去 1 体力",
  qianjie: "锁定：不能进入连环（横置）状态；不能成为延时锦囊与拼点目标",
  jueyan: "出牌阶段限一次：废除一个装备栏并获得本回合效果（武器/防具/坐骑/宝物 四条分支）",
  poshi: "觉醒：准备阶段装备栏全废除或体力=1 → 减 1 体力上限、摸至上限、失去决堰获得怀柔",
  xiongluan: "限定技：出牌阶段指定一名其他角色 → 废除自己判定区与全部装备栏；本回合对其无距离/次数限制，其不能使用/打出手牌",
  congjian: "成为目标数 >1 的锦囊目标后 → 交给其中一名目标一张牌，然后摸 1（装备牌摸 2）",
};

function parseSkillsMd(text) {
  const map = new Map();
  const re = /^\|\s*[^|]*?`(sgs_hero_\d+)`\s*\|\s*([^|]+?)\s*\|\s*`([a-z_0-9]+)`\s*\|\s*([^|]+?)\s*\|(.*)\|\s*$/;
  for (const line of text.split("\n")) {
    const m = re.exec(line.trim());
    if (!m) continue;
    const [, generalCode, name, code, kindZh, effect] = m;
    const key = `${generalCode}::${code}`;
    if (map.has(key)) continue;
    map.set(key, { name: name.trim(), kindZh: kindZh.trim(), effect: effect.trim() });
  }
  return map;
}

async function readAudit(batchId) {
  const file = path.join(AUDIT_DIR, `${batchId}.json`);
  try {
    return JSON.parse(await fs.readFile(file, "utf8"));
  } catch {
    return null;
  }
}

/** `--no-scene` 机检（保留开局 pending）：`data/audit/<batch>-noscene.json`。 */
async function readAuditNoScene(batchId) {
  const file = path.join(AUDIT_DIR, `${batchId}-noscene.json`);
  try {
    return JSON.parse(await fs.readFile(file, "utf8"));
  } catch {
    return null;
  }
}

function invLine(g) {
  const inv = g?.invariants ?? {};
  const mark = (v) => (v ? "✓" : "✗");
  return `noError=${mark(inv.noError)} conservation=${mark(inv.conservation)} pendingContinuable=${mark(inv.pendingContinuable)} deterministic=${mark(inv.deterministic)}`;
}

function renderSheet(batch, catalog, skills, audit, seed, auditNoScene = null) {
  const generals = batch.generals;
  const auditByCode = new Map((audit?.generals ?? []).map((g) => [g.code, g]));
  const nsByCode = new Map((auditNoScene?.generals ?? []).map((g) => [g.code, g]));
  const skillCount = generals.reduce((n, g) => n + g.skills.length, 0);
  const reached = generals.reduce(
    (n, g) => n + g.skills.filter((s) => (auditByCode.get(g.code)?.reachedSkills ?? []).includes(s.code)).length,
    0,
  );
  const failed = (audit?.generals ?? []).filter((g) => g.status === "失败" || g.status === "阻塞");
  const L = [];
  L.push(`# ${batch.id} 走查单 · ${batch.source}（${generals.length} 将 / ${skillCount} 技能）`);
  L.push("");
  L.push("> 生成：`apps/scripts/sgs-audit-sheet.mjs`（**勿手改本文件**；结论写回矩阵）。");
  L.push("> 本单只含：`skills.md` 原文、技能类型、机器证据（可达/不报错/守恒/pending/可复现）。");
  L.push("> **`expected`/结论/确认人只能由人工填写**：agent 不得标「人工通过」（`" + MATRIX_REL + "` 头注）。");
  L.push("");
  L.push("## 0. 批次信息");
  L.push("");
  L.push(`- 批次：\`${batch.id}\` · 来源：${batch.source} · 将数 ${generals.length} / 技能 ${skillCount}`);
  L.push(`- 复现：\`node apps/scripts/sgs-general-audit.mjs --batch ${batch.id} --seed ${seed} --repeat 2\``);
  L.push(`- 机检 JSON：\`data/audit/${batch.id}.json\`${audit ? `（生成于 ${audit.generatedAt}）` : "（**缺失**，先跑上面命令）"}`);
  if (audit) {
    L.push(
      `- 机检汇总：触达技能 ${reached}/${skillCount} · 失败 ${failed.length} · 真相源未写 ${audit.summary?.truthSourceUntouched ? "✓" : "✗"}`,
    );
    if (auditNoScene) {
      const nsHit = generals.reduce(
        (n, g) =>
          n + g.skills.filter((s) => (nsByCode.get(g.code)?.reachedSkills ?? []).includes(s.code)).length,
        0,
      );
      L.push(
        `- no-scene 机检（保留开局 pending，\`data/audit/${batch.id}-noscene.json\`）：触达技能 ${nsHit}/${skillCount} · 失败 ${auditNoScene.summary?.failed ?? 0}`,
      );
    }
    if (failed.length) {
      L.push("");
      L.push("> ⚠️ 机检失败项（先修再看语义）：");
      for (const g of failed) L.push(`> - ${g.name} \`${g.code}\`：${g.status} — ${g.reason ?? ""}`);
    }
  }
  L.push("");
  L.push("## 1. 走查流程（每将）");
  L.push("");
  L.push("```");
  L.push("工作台 → 对局预览/沙盒（指定武将+身份+seed）→ 视角切到目标玩家");
  L.push("→ 按「场景提示」调整手牌/装备/阶段 → 触发技能 → 逐条对照「判定点」");
  L.push("→ 记录：实际操作 / 期望结果 / 实际结果 / 结论 / 确认人+日期");
  L.push("```");
  L.push("");
  L.push(`规则口径与既有裁定：\`docs/games/sanguosha/rules-rulings.md\`；实现细节：\`docs/games/sanguosha/skills.md\`。`);
  L.push("");

  for (const g of generals) {
    const a = auditByCode.get(g.code);
    const ns = nsByCode.get(g.code);
    L.push(`## ${g.name} \`${g.code}\`（${g.pack}）`);
    L.push("");
    L.push(
      `- 机检：${a ? `${a.status}${a.reachedSkills?.length ? ` · 触达 ${a.reachedSkills.join(",")}` : ""}` : "无证据"} · ${invLine(a)}`,
    );
    if (ns && (ns.reachedSkills ?? []).length) {
      L.push(`- no-scene 机检：${ns.status} · 触达 ${ns.reachedSkills.join(",")}`);
    }
    if (a?.untrackedSkills?.length) {
      L.push(`- L1 未触达：${a.untrackedSkills.map((s) => `${s.name}(\`${s.code}\`)`).join("、")}`);
    }
    if (g.skills.some((s) => OPENING_SKILLS.has(s.code))) {
      L.push(
        `- **开局技**：本将有开局钩子（${g.skills
          .filter((s) => OPENING_SKILLS.has(s.code))
          .map((s) => s.name)
          .join("/")}）→ 预览「注入场景」会丢弃开局 pending，走查/机检请先处理它或用 \`npm run audit:sgs-generals:no-scene ${batch.id}\``,
      );
    }
    L.push(`- 场景：5 人局 · 目标座位 0 · seed=${seed} · 阶段按技能（默认出牌阶段）`);
    L.push("");
    L.push("| 技能 | code | 类型 | 原文（skills.md） | 机检 |");
    L.push("|---|---|---|---|---|");
    for (const s of g.skills) {
      const md = skills.get(`${g.code}::${s.code}`) ?? skills.get(`*::${s.code}`);
      const text = md ? md.effect.replace(/\|/g, "\\|") : "**skills.md 未登记原文**";
      const hit = (a?.reachedSkills ?? []).includes(s.code);
      const nsHit = (nsByCode.get(g.code)?.reachedSkills ?? []).includes(s.code);
      const cell = hit ? "触达" : nsHit ? "未触达（no-scene 触达）" : "未触达";
      L.push(`| ${s.name} | \`${s.code}\` | ${KIND_ZH[s.kind] ?? s.kind} | ${text} | ${cell} |`);
    }
    L.push("");
    const kinds = [...new Set(g.skills.map((s) => s.kind))];
    for (const kind of kinds) {
      L.push(`**判定点（${KIND_ZH[kind] ?? kind}）**`);
      for (const c of KIND_CHECKS[kind] ?? KIND_CHECKS.normal) L.push(`- [ ] ${c}`);
      L.push("");
    }
    const hints = g.skills.map((s) => SCENE_HINTS[s.code]).filter(Boolean);
    if (hints.length) {
      L.push("**场景构造提示（只描述条件，不判语义）**");
      for (const h of hints) L.push(`- ${h}`);
      L.push("");
    }
    L.push("**结论（人工填写）**");
    L.push("");
    L.push("| 技能 | 实际操作 | 期望结果 | 实际结果 | 结论 | 确认人+日期 |");
    L.push("|---|---|---|---|---|---|");
    for (const s of g.skills) L.push(`| ${s.name}(\`${s.code}\`) | | | | 待走查 | |`);
    L.push("");
  }

  L.push("## 2. 批次签字（人工）");
  L.push("");
  L.push("| 项 | 值 |");
  L.push("|---|---|");
  L.push(`| 批次 | ${batch.id} |`);
  L.push("| 通过 / 失败 / 待裁定 | / / |");
  L.push("| 确认人 | |");
  L.push("| 日期 | |");
  L.push("| 备注（缺陷号/裁定链接） | |");
  L.push("");
  L.push(`签字后把结论写回矩阵：\`${MATRIX_REL}\`（第 3 节对应行的人工结果/确认人/备注列）。`);
  L.push("");
  return `${L.join("\n")}\n`;
}

async function main() {
  const catalog = loadCatalog();
  const batches = expandBatches(catalog);
  const skillsText = await fs.readFile(SKILLS_MD, "utf8").catch(() => "");
  const skills = parseSkillsMd(skillsText);
  const wanted = flag("--all") ? batches.map((b) => b.id) : opts("--batch");
  if (!wanted.length) throw new Error("需要 --batch <B0..B20>（可多次）或 --all");
  const seed = Number(opt("--seed", 20261105));
  await fs.mkdir(OUT_DIR, { recursive: true });
  for (const id of wanted) {
    const batch = batches.find((b) => b.id === id.toUpperCase());
    if (!batch) throw new Error(`未知批次：${id}`);
    const audit = await readAudit(batch.id);
    const auditNoScene = await readAuditNoScene(batch.id);
    const md = renderSheet(batch, catalog, skills, audit, seed, auditNoScene);
    const file = path.join(OUT_DIR, `${batch.id}.md`);
    await fs.writeFile(file, md, "utf8");
    const n = batch.generals.reduce((k, g) => k + g.skills.length, 0);
    console.log(
      `[sgs-sheet] ${batch.id}：${batch.generals.length} 将 / ${n} 技能 → ${path.relative(projectRoot, file)}${audit ? "" : "（机检证据缺失）"}`,
    );
  }
  return 0;
}

main().then(
  (code) => process.exit(code ?? 0),
  (e) => {
    console.error(`[sgs-sheet] ❌ ${e?.message ?? e}`);
    process.exit(1);
  },
);
