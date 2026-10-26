<script setup lang="ts">
import { computed, h, onMounted, ref, watch } from "vue";
import { useRoute } from "vue-router";
import {
  NButton,
  NDataTable,
  NForm,
  NFormItem,
  NInput,
  NInputNumber,
  NModal,
  NSelect,
  NSpace,
  NTag,
  useMessage,
  type DataTableColumns,
} from "naive-ui";
import { api } from "../api";
import { emptyRow, schemas, tableColumns, type Field, type ResourceSchema } from "../schemas";

const route = useRoute();
const message = useMessage();
const table = computed(() => String(route.meta.table ?? ""));
const schema = computed<ResourceSchema | undefined>(() => schemas[table.value]);

const rows = ref<Record<string, unknown>[]>([]);
const loading = ref(false);
const search = ref("");
const status = ref<string | null>(null);

const showModal = ref(false);
const saving = ref(false);
const form = ref<Record<string, unknown>>({});
const editing = ref(false);

const statusOptions = [
  { label: "全部状态", value: "" },
  { label: "新建", value: "draft" },
  { label: "测试", value: "testing" },
  { label: "审核", value: "review" },
  { label: "上线", value: "live" },
  { label: "下线", value: "offline" },
];

async function load() {
  if (!schema.value) return;
  loading.value = true;
  try {
    const res = await api.list(table.value, {
      q: search.value.trim() || undefined,
      status: status.value || undefined,
      limit: 500,
    });
    rows.value = res.rows;
  } catch (e) {
    message.error((e as Error).message);
  } finally {
    loading.value = false;
  }
}

function rowKey(row: Record<string, unknown>): string {
  if (row.id !== undefined && row.id !== null) return String(row.id);
  if (row.code) return String(row.code);
  return `${row.general_code ?? ""}:${row.skill_code ?? ""}`;
}

function entityCode(row: Record<string, unknown>): string {
  return row.id !== undefined && row.id !== null ? String(row.id) : String(row.code ?? "");
}

function openCreate() {
  if (!schema.value) return;
  form.value = emptyRow(schema.value);
  editing.value = false;
  showModal.value = true;
}

function openEdit(row: Record<string, unknown>) {
  form.value = { ...row };
  editing.value = true;
  showModal.value = true;
}

async function save() {
  if (!schema.value) return;
  saving.value = true;
  try {
    await api.upsert(table.value, form.value);
    message.success("已保存");
    showModal.value = false;
    await load();
  } catch (e) {
    message.error((e as Error).message);
  } finally {
    saving.value = false;
  }
}

async function moveStatus(row: Record<string, unknown>, to: string) {
  try {
    await api.setStatus(table.value, entityCode(row), to);
    message.success(`已置为 ${to}`);
    await load();
  } catch (e) {
    message.error((e as Error).message);
  }
}

const columns = computed<DataTableColumns<Record<string, unknown>>>(() => {
  if (!schema.value) return [];
  const cols: DataTableColumns<Record<string, unknown>> = tableColumns(schema.value).map((f) => ({
    title: f.label,
    key: f.key,
    width: f.width,
    ellipsis: true,
    render: (row) => {
      const v = row[f.key];
      if (f.key === "status") {
        return h(NTag, { size: "small", type: v === "live" ? "success" : v === "offline" ? "error" : "default" }, { default: () => String(v ?? "") });
      }
      return v === null || v === undefined || v === "" ? "-" : String(v);
    },
  }));
  cols.push({
    title: "操作",
    key: "__actions",
    width: 260,
    render: (row) => {
      const buttons = [
        h(NButton, { size: "tiny", onClick: () => openEdit(row) }, { default: () => "编辑" }),
      ];
      if (schema.value?.fields.some((f) => f.key === "status")) {
        buttons.push(
          h(NButton, { size: "tiny", onClick: () => moveStatus(row, "review") }, { default: () => "审核" }),
          h(NButton, { size: "tiny", type: "primary", onClick: () => moveStatus(row, "live") }, { default: () => "上线" }),
          h(NButton, { size: "tiny", type: "error", onClick: () => moveStatus(row, "offline") }, { default: () => "下线" }),
        );
      }
      return h(NSpace, { size: 4 }, { default: () => buttons });
    },
  });
  return cols;
});

function fieldValue(f: Field): unknown {
  return form.value[f.key] ?? (f.type === "number" ? null : "");
}

function setField(f: Field, v: unknown) {
  form.value[f.key] = v;
}

onMounted(load);
watch(() => route.path, load);
</script>

<template>
  <div v-if="schema">
    <div style="display: flex; align-items: center; gap: 12px; margin-bottom: 16px">
      <h2 style="margin: 0">{{ schema.title }}</h2>
      <span style="color: #888; font-size: 12px">{{ table }}</span>
      <div style="flex: 1" />
      <n-input v-model:value="search" placeholder="搜索 code / 名称" clearable style="width: 220px" @keyup.enter="load" />
      <n-select v-model:value="status" :options="statusOptions" style="width: 140px" clearable placeholder="状态" @update:value="load" />
      <n-button @click="load">刷新</n-button>
      <n-button type="primary" @click="openCreate">新建</n-button>
    </div>

    <n-data-table
      :columns="columns"
      :data="rows"
      :loading="loading"
      :row-key="rowKey"
      :max-height="640"
      virtual-scroll
      size="small"
      striped
    />

    <n-modal v-model:show="showModal" preset="card" :title="editing ? `编辑 ${schema.title}` : `新建 ${schema.title}`" style="width: 720px">
      <n-form label-placement="top" :model="form">
        <n-form-item v-for="f in schema.fields" :key="f.key" :label="f.label" :path="f.key">
          <n-select
            v-if="f.type === 'select'"
            :value="(fieldValue(f) as string | number | null)"
            :options="f.options ?? []"
            clearable
            @update:value="(v: string | number | null) => setField(f, v)"
          />
          <n-input-number
            v-else-if="f.type === 'number'"
            :value="(fieldValue(f) as number | null)"
            style="width: 100%"
            @update:value="(v: number | null) => setField(f, v)"
          />
          <n-input
            v-else
            :value="(fieldValue(f) as string)"
            :type="f.type === 'textarea' ? 'textarea' : 'text'"
            :autosize="f.type === 'textarea' ? { minRows: 2, maxRows: 6 } : undefined"
            @update:value="(v: string) => setField(f, v)"
          />
        </n-form-item>
      </n-form>
      <template #footer>
        <n-space justify="end">
          <n-button @click="showModal = false">取消</n-button>
          <n-button type="primary" :loading="saving" @click="save">保存</n-button>
        </n-space>
      </template>
    </n-modal>
  </div>
</template>
