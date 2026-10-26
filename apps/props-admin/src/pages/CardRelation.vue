<script setup lang="ts">
import { computed, h, onMounted, ref } from "vue";
import {
  NButton,
  NCard,
  NDataTable,
  NForm,
  NFormItem,
  NInput,
  NInputNumber,
  NSelect,
  NSpace,
  NTag,
  useMessage,
  type DataTableColumns,
} from "naive-ui";
import { api } from "../api";

const message = useMessage();
const defs = ref<Record<string, unknown>[]>([]);
const cards = ref<Record<string, unknown>[]>([]);
const selectedDef = ref<string | null>(null);
const loading = ref(false);

const form = ref<Record<string, unknown>>({
  pack_id: 1,
  def_code: "",
  suit: null,
  rank: null,
});

const defOptions = computed(() =>
  defs.value.map((d) => ({ label: `${d.name}（${d.code}）`, value: String(d.code) })),
);

const suitOptions = [
  { label: "黑桃", value: "spade" },
  { label: "红桃", value: "heart" },
  { label: "梅花", value: "club" },
  { label: "方块", value: "diamond" },
];

async function loadDefs() {
  const res = await api.list("sgs_card_defs", { limit: 500 });
  defs.value = res.rows;
}

async function loadCards() {
  if (!selectedDef.value) {
    cards.value = [];
    return;
  }
  loading.value = true;
  try {
    const res = await api.list("sgs_cards", { q: selectedDef.value, limit: 500 });
    cards.value = res.rows.filter((r) => r.def_code === selectedDef.value);
  } catch (e) {
    message.error((e as Error).message);
  } finally {
    loading.value = false;
  }
}

async function addCard() {
  if (!selectedDef.value) {
    message.warning("先选择牌定义");
    return;
  }
  try {
    await api.upsert("sgs_cards", {
      ...form.value,
      def_code: selectedDef.value,
      status: "draft",
    });
    message.success("已添加物理牌");
    await loadCards();
  } catch (e) {
    message.error((e as Error).message);
  }
}

async function moveStatus(row: Record<string, unknown>, to: string) {
  try {
    await api.setStatus("sgs_cards", String(row.id), to);
    message.success(`已置为 ${to}`);
    await loadCards();
  } catch (e) {
    message.error((e as Error).message);
  }
}

const columns = computed<DataTableColumns<Record<string, unknown>>>(() => [
  { title: "ID", key: "id", width: 80 },
  { title: "包 ID", key: "pack_id", width: 90 },
  { title: "牌定义", key: "def_code", width: 140 },
  { title: "花色", key: "suit", width: 90 },
  { title: "点数", key: "rank", width: 80 },
  {
    title: "状态",
    key: "status",
    width: 90,
    render: (row) => h(NTag, { size: "small" }, { default: () => String(row.status ?? "-") }),
  },
  {
    title: "操作",
    key: "__actions",
    width: 220,
    render: (row) =>
      h(NSpace, { size: 4 }, {
        default: () => [
          h(NButton, { size: "tiny", onClick: () => moveStatus(row, "review") }, { default: () => "审核" }),
          h(NButton, { size: "tiny", type: "primary", onClick: () => moveStatus(row, "live") }, { default: () => "上线" }),
          h(NButton, { size: "tiny", type: "error", onClick: () => moveStatus(row, "offline") }, { default: () => "下线" }),
        ],
      }),
  },
]);

onMounted(async () => {
  try {
    await loadDefs();
  } catch (e) {
    message.error((e as Error).message);
  }
});
</script>

<template>
  <div>
    <h2 style="margin-top: 0">牌定义 ↔ 物理牌</h2>
    <p style="color: #888">
      选中一个牌定义，查看/新增它对应的物理牌（花色 + 点数）；物理牌初始为「新建」，人工复核后上线。
    </p>

    <n-card size="small" style="margin-bottom: 16px">
      <n-space align="end">
        <n-form-item label="牌定义" style="margin-bottom: 0">
          <n-select
            v-model:value="selectedDef"
            :options="defOptions"
            filterable
            placeholder="选择牌定义"
            style="width: 320px"
            @update:value="loadCards"
          />
        </n-form-item>
        <n-button @click="loadCards">刷新</n-button>
      </n-space>
    </n-card>

    <n-card v-if="selectedDef" size="small" title="新增物理牌" style="margin-bottom: 16px">
      <n-form inline label-placement="top" :model="form">
        <n-form-item label="包 ID">
          <n-input-number v-model:value="(form.pack_id as number)" :min="1" style="width: 100px" />
        </n-form-item>
        <n-form-item label="花色">
          <n-select v-model:value="(form.suit as string)" :options="suitOptions" style="width: 120px" clearable />
        </n-form-item>
        <n-form-item label="点数（A=1）">
          <n-input-number v-model:value="(form.rank as number)" :min="1" :max="13" style="width: 120px" />
        </n-form-item>
        <n-form-item label=" ">
          <n-button type="primary" @click="addCard">添加</n-button>
        </n-form-item>
      </n-form>
      <p style="color: #888; font-size: 12px">
        包 ID 需先在「道具包」中创建（标准/军争牌堆包）；花色/点数可留空，后续补充。
      </p>
    </n-card>

    <n-data-table
      :columns="columns"
      :data="cards"
      :loading="loading"
      :row-key="(row: Record<string, unknown>) => String(row.id)"
      size="small"
      :max-height="500"
      virtual-scroll
      striped
    />
  </div>
</template>
