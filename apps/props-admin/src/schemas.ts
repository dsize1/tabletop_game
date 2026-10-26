export type FieldType = "text" | "number" | "textarea" | "select";

export interface Field {
  key: string;
  label: string;
  type?: FieldType;
  options?: { label: string; value: string | number }[];
  required?: boolean;
  hideInTable?: boolean;
  width?: number;
}

export interface ResourceSchema {
  table: string;
  title: string;
  fields: Field[];
  readOnly?: boolean;
}

const STATUS = [
  { label: "新建", value: "draft" },
  { label: "测试", value: "testing" },
  { label: "审核", value: "review" },
  { label: "上线", value: "live" },
  { label: "下线", value: "offline" },
] as const;

const statusField: Field = {
  key: "status",
  label: "状态",
  type: "select",
  options: [...STATUS],
  required: true,
};

const timestamps: Field[] = [
  { key: "created_at", label: "创建时间", hideInTable: true },
  { key: "updated_at", label: "更新时间", hideInTable: true },
];

export const schemas: Record<string, ResourceSchema> = {
  "prop_packs": {
    table: "prop_packs",
    title: "道具包",
    fields: [
      { key: "id", label: "ID", type: "number", hideInTable: true },
      { key: "game_id", label: "游戏", required: true },
      { key: "kind", label: "类型", required: true },
      { key: "code", label: "code", required: true },
      { key: "name", label: "名称", required: true },
      { key: "sort_order", label: "排序", type: "number" },
      { key: "enabled", label: "启用", type: "number" },
      ...timestamps,
    ],
  },
  "sgs_identities": {
    table: "sgs_identities",
    title: "身份",
    fields: [
      { key: "code", label: "code", required: true },
      { key: "name", label: "名称", required: true },
      {
        key: "team",
        label: "阵营",
        type: "select",
        options: [
          { label: "主", value: "lord" },
          { label: "反", value: "rebel" },
          { label: "内", value: "spy" },
        ],
        required: true,
      },
      { key: "win_condition", label: "胜利条件", type: "textarea", required: true },
      { key: "description", label: "描述", type: "textarea" },
      { key: "sort_order", label: "排序", type: "number" },
      statusField,
      ...timestamps,
    ],
  },
  "sgs_identity_setup": {
    table: "sgs_identity_setup",
    title: "身份配比",
    fields: [
      { key: "player_count", label: "人数", type: "number", required: true },
      {
        key: "variant",
        label: "变体",
        type: "select",
        options: [
          { label: "标准", value: "default" },
          { label: "双内", value: "double_spy" },
        ],
        required: true,
      },
      { key: "identity_code", label: "身份", required: true },
      { key: "count", label: "数量", type: "number", required: true },
    ],
  },
  "sgs_card_defs": {
    table: "sgs_card_defs",
    title: "牌定义",
    fields: [
      { key: "code", label: "code", required: true },
      { key: "name", label: "名称", required: true },
      {
        key: "category",
        label: "分类",
        type: "select",
        options: [
          { label: "基本牌", value: "basic" },
          { label: "武器", value: "weapon" },
          { label: "护具", value: "armor" },
          { label: "坐骑", value: "horse" },
          { label: "特殊装备", value: "treasure" },
          { label: "锦囊", value: "trick" },
          { label: "延时锦囊", value: "delayed_trick" },
        ],
        required: true,
      },
      { key: "description", label: "描述", type: "textarea" },
      { key: "range_value", label: "距离值", type: "number" },
      {
        key: "target_mode",
        label: "目标",
        type: "select",
        options: [
          { label: "单目标", value: "single" },
          { label: "AOE", value: "aoe" },
        ],
      },
      {
        key: "equip_slot",
        label: "装备槽",
        type: "select",
        options: [
          { label: "武器", value: "weapon" },
          { label: "护具", value: "armor" },
          { label: "进攻马", value: "horse_offense" },
          { label: "防御马", value: "horse_defense" },
          { label: "特殊", value: "treasure" },
        ],
      },
      {
        key: "range_type",
        label: "马类型",
        type: "select",
        options: [
          { label: "进攻马", value: "attack" },
          { label: "防御马", value: "defense" },
        ],
      },
      statusField,
      ...timestamps,
    ],
  },
  "sgs_cards": {
    table: "sgs_cards",
    title: "物理牌",
    fields: [
      { key: "id", label: "ID", type: "number" },
      { key: "pack_id", label: "包 ID", type: "number", required: true },
      { key: "def_code", label: "牌定义", required: true },
      {
        key: "suit",
        label: "花色",
        type: "select",
        options: [
          { label: "黑桃", value: "spade" },
          { label: "红桃", value: "heart" },
          { label: "梅花", value: "club" },
          { label: "方块", value: "diamond" },
        ],
      },
      { key: "rank", label: "点数", type: "number" },
      statusField,
      ...timestamps,
    ],
  },
  "sgs_generals": {
    table: "sgs_generals",
    title: "武将",
    fields: [
      { key: "code", label: "code", required: true },
      { key: "external_id", label: "外部 ID", hideInTable: true },
      { key: "name", label: "名称", required: true },
      {
        key: "kingdom",
        label: "势力",
        type: "select",
        options: [
          { label: "魏", value: "wei" },
          { label: "蜀", value: "shu" },
          { label: "吴", value: "wu" },
          { label: "群", value: "qun" },
          { label: "晋", value: "jin" },
          { label: "神", value: "god" },
        ],
        required: true,
      },
      {
        key: "gender",
        label: "性别",
        type: "select",
        options: [
          { label: "男", value: "male" },
          { label: "女", value: "female" },
          { label: "未知", value: "unknown" },
        ],
      },
      { key: "pack_id", label: "包 ID", type: "number", required: true },
      { key: "max_hp", label: "最高体力", type: "number", required: true },
      { key: "initial_hp", label: "初始体力", type: "number" },
      { key: "quality", label: "品质" },
      { key: "description", label: "特点", type: "textarea", hideInTable: true },
      { key: "biography", label: "生平", type: "textarea", hideInTable: true },
      statusField,
      ...timestamps,
    ],
  },
  "sgs_skills": {
    table: "sgs_skills",
    title: "技能",
    fields: [
      { key: "code", label: "code", required: true },
      { key: "external_id", label: "外部 ID", hideInTable: true },
      { key: "name", label: "名称", required: true },
      {
        key: "type",
        label: "类型",
        type: "select",
        options: [
          { label: "主公技", value: "lord" },
          { label: "一般", value: "normal" },
          { label: "锁定技", value: "locked" },
          { label: "限定技", value: "limited" },
          { label: "觉醒技", value: "awakening" },
        ],
        required: true,
      },
      { key: "description", label: "描述", type: "textarea", hideInTable: true },
      statusField,
      ...timestamps,
    ],
  },
  "sgs_general_skills": {
    table: "sgs_general_skills",
    title: "武将技能关联",
    fields: [
      { key: "general_code", label: "武将", required: true },
      { key: "skill_code", label: "技能", required: true },
      { key: "sort_order", label: "排序", type: "number" },
    ],
  },
};

export function emptyRow(schema: ResourceSchema): Record<string, unknown> {
  const row: Record<string, unknown> = {};
  for (const f of schema.fields) {
    if (f.key === "status") row[f.key] = "draft";
    else if (f.type === "number") row[f.key] = f.required ? 0 : null;
    else row[f.key] = "";
  }
  return row;
}

export function tableColumns(schema: ResourceSchema): Field[] {
  return schema.fields.filter((f) => !f.hideInTable).slice(0, 8);
}
