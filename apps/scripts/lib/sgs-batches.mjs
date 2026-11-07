// 全武将验证批次表（单一事实源；矩阵/审计/门禁/文档均以此为准）。
//
// 口径（用户 2026-11-05 裁定「方案 A」）：
// - 有界版同名武将的原版不进验证批次（界优先去重），保留 148 位 = 默认选将池（232 − 84）；
// - 批次按「子包边界」切分，每批 ≤8 将；B0 为流程验证批（3 将）。
//
// 校验：`node apps/scripts/sgs-general-audit.mjs --list` 会核对该表 ↔ data.rs（148 位、无重无漏）。
// 注意：不要在此手写人数；`npm run check:sgs-counts` 会校验文档/脚本里的计数与 catalog 一致。

export const BATCHES = [
  { id: "B0", source: "风林火山·风/林/山（保留）", names: ["曹仁", "周泰", "曹丕"] },
  {
    id: "B1",
    source: "风林火山·SP-1",
    names: ["袁术", "王基", "严颜", "王平", "陆绩", "孙亮", "蒯越蒯良", "许攸"],
  },
  {
    id: "B2",
    source: "风林火山·SP-2",
    names: ["卢植", "郝昭", "陈到", "诸葛瞻", "周妃", "毌丘俭", "陆抗", "张绣"],
  },
  {
    id: "B3",
    source: "一将成名·批次1/2/Y1（保留）",
    names: ["马谡", "荀攸", "将华雄", "钟会", "满宠", "关平"],
  },
  {
    id: "B4",
    source: "一将成名·Y2+Y3（保留）",
    names: ["刘封", "潘璋马忠", "朱然", "曹真", "韩浩史涣", "陈群", "吴懿", "周仓"],
  },
  { id: "B5", source: "一将成名·Y4（保留）", names: ["孙鲁班", "朱桓", "顾雍", "曹叡", "曹休", "钟繇"] },
  {
    id: "B6",
    source: "一将成名·Y5（保留）",
    names: ["刘谌", "张嶷", "孙休", "朱治", "全琮", "公孙渊", "郭图逢纪"],
  },
  { id: "B7", source: "一将成名·Y6（保留）", names: ["李严", "孙登", "刘虞", "岑昏", "孙资刘放", "黄皓", "张让"] },
  {
    id: "B8",
    source: "一将成名·Y7（保留）",
    names: ["辛宪英", "吴苋", "徐氏", "曹节", "嵇康", "秦宓", "薛综", "蔡邕"],
  },
  {
    id: "B9",
    source: "界限突破·界标·蜀",
    names: ["界刘备", "界关羽", "界张飞", "界赵云", "界马超", "界诸葛亮", "界黄月英"],
  },
  {
    id: "B10",
    source: "界限突破·界标·吴",
    names: ["界甘宁", "界吕蒙", "界黄盖", "界周瑜", "界大乔", "界陆逊", "界孙权", "界孙尚香"],
  },
  {
    id: "B11",
    source: "界限突破·界标·魏",
    names: ["界曹操", "界司马懿", "界夏侯惇", "界张辽", "界许褚", "界郭嘉", "界李典", "界甄姬"],
  },
  { id: "B12", source: "界限突破·界标·群", names: ["界华佗", "界吕布", "界公孙瓒", "界貂蝉"] },
  {
    id: "B13",
    source: "界限突破·J1",
    names: ["界黄忠", "界徐庶", "界华雄", "界左慈", "界张角", "界于吉", "界袁绍", "界刘禅"],
  },
  {
    id: "B14",
    source: "界限突破·J2",
    names: [
      "界孙策",
      "界夏侯渊",
      "界卧龙诸葛亮",
      "界庞统",
      "界魏延",
      "界小乔",
      "界孙坚",
      "界庞德",
    ],
  },
  {
    id: "B15",
    source: "界限突破·J3",
    names: ["界太史慈", "界董卓", "界邓艾", "界祝融", "界姜维", "界贾诩", "界鲁肃", "界张郃"],
  },
  {
    id: "B16",
    source: "界限突破·J4",
    names: [
      "界孟获",
      "界徐晃",
      "界蔡文姬",
      "界荀彧",
      "界典韦",
      "界颜良文丑",
      "界张昭张纮",
      "界徐盛",
    ],
  },
  {
    id: "B17",
    source: "界限突破·J5",
    names: ["界凌统", "界陈宫", "界曹彰", "界简雍", "界虞翻", "界高顺", "界法正", "界程普"],
  },
  {
    id: "B18",
    source: "界限突破·J6",
    names: ["界王异", "界张春华", "界刘表", "界蔡夫人", "界曹冲", "界曹植", "界吴国太", "界廖化"],
  },
  {
    id: "B19",
    source: "界限突破·J7",
    names: [
      "界李儒",
      "界郭淮",
      "界伏皇后",
      "界韩当",
      "界关兴张苞",
      "界马岱",
      "界郭皇后",
      "界沮授",
    ],
  },
  { id: "B20", source: "界限突破·J8", names: ["界夏侯氏", "界张松", "界于禁", "界步练师"] },
];

export const MAX_PER_BATCH = 8;

/** 展开批次为 [{ batch, source, names }]，并做重/漏/超限校验。 */
export function expandBatches(catalog) {
  const seen = new Map();
  const out = [];
  for (const b of BATCHES) {
    if (b.names.length > MAX_PER_BATCH) {
      throw new Error(`批次 ${b.id} 超过 ${MAX_PER_BATCH} 将（${b.names.length}）`);
    }
    for (const name of b.names) {
      if (seen.has(name)) {
        throw new Error(`武将「${name}」重复出现在 ${seen.get(name)} 与 ${b.id}`);
      }
      seen.set(name, b.id);
    }
    const byName = new Map(catalog.audited.map((g) => [g.name, g]));
    const generals = b.names.map((name) => {
      const g = byName.get(name);
      if (!g) {
        const ex = catalog.excluded.find((x) => x.name === name);
        throw new Error(
          ex
            ? `武将「${name}」有界版同名，按口径已排除，不应出现在批次 ${b.id}`
            : `武将「${name}」不在 148 审计范围内（批次 ${b.id}）`,
        );
      }
      return g;
    });
    out.push({ ...b, generals });
  }
  const missing = catalog.audited.filter((g) => !seen.has(g.name));
  if (missing.length) {
    throw new Error(
      `批次表漏掉 ${missing.length} 位武将：${missing.map((g) => g.name).join("、")}`,
    );
  }
  return out;
}

/** 取批次（支持 B0 / b0 / 0）。 */
export function findBatch(batchId) {
  const norm = String(batchId).trim().toUpperCase().replace(/^B?/, "B");
  return BATCHES.find((b) => b.id === norm) ?? null;
}
