// 三国杀官方数据勘误表（单一事实源）。
//
// 官网抓取的 `docs/games/sanguosha/data/heroes.json` 偶有错别字；
// 这些名字会流入 `data.rs`（引擎）、`generals.md`（文档）与验证矩阵/批次表。
// 典型后果：`prefer_jiexian`（界优先去重）按名字比对 → 错别字导致原版与界版无法识别为同一武将，
// 原版错误进池并被当作独立武将验证（如 gid=55「张昭张弘」与「界张昭张纮」）。
//
// 约定：**不改 heroes.json 原始抓取结果**，在生成/校验时统一应用本表，保证可复现与可追溯。
export const GENERAL_NAME_FIXES = {
  55: "张昭张纮",
};

/** 应用勘误表后的武将名。 */
export function fixedGeneralName(hero) {
  return GENERAL_NAME_FIXES[hero.gid] ?? hero.name;
}

/** 勘误记录（文档/报告用）。 */
export const GENERAL_NAME_FIX_NOTES = [
  { gid: 55, from: "张昭张弘", to: "张昭张纮", reason: "与「界张昭张纮」同名将；弘为错别字" },
];
