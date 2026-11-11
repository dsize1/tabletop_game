<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import {
  NButton,
  NCard,
  NCheckbox,
  NDivider,
  NDrawer,
  NDrawerContent,
  NForm,
  NFormItem,
  NInput,
  NInputNumber,
  NSelect,
  NSpace,
  NTabPane,
  NTabs,
  NTag,
  useDialog,
  useMessage,
} from "naive-ui";
import {
  logApi,
  previewApi,
  templateScene,
  workbenchApi,
  type DecisionLogRow,
  type PreviewSeat,
  type PreviewState,
  type PreviewStep,
  type SandboxMeta,
  type TemplateDetail,
  type TemplateRow,
} from "../api";

const message = useMessage();
const dialog = useDialog();
const tab = ref("templates");

// 模板
const templates = ref<TemplateRow[]>([]);
const selectedId = ref<number | null>(null);
const detail = ref<TemplateDetail | null>(null);
const templateName = ref("");
const sceneText = ref("{}");
const importText = ref("");
const grabRoom = ref("");
const loading = ref(false);
const detailTab = ref("scene");

async function loadTemplates() {
  loading.value = true;
  try {
    const res = await workbenchApi.templates();
    templates.value = res.rows;
    if (selectedId.value === null && res.rows.length > 0) {
      await selectTemplate(res.rows[0]);
    }
  } catch (e) {
    message.error((e as Error).message);
  } finally {
    loading.value = false;
  }
}

async function selectTemplate(row: { id: number }) {
  selectedId.value = row.id;
  try {
    const d = await workbenchApi.template(row.id);
    detail.value = d;
    templateName.value = d.name;
    sceneText.value = JSON.stringify(templateScene(d.meta) ?? {}, null, 2);
  } catch (e) {
    message.error((e as Error).message);
  }
}

function parseJson(text: string, label: string): unknown | undefined {
  try {
    return text.trim() ? JSON.parse(text) : {};
  } catch (e) {
    message.error(`${label} 不是合法 JSON：${(e as Error).message}`);
    return undefined;
  }
}

async function saveScene() {
  if (!detail.value) return;
  const scene = parseJson(sceneText.value, "场景");
  if (scene === undefined) return;
  try {
    const meta = { ...((detail.value.meta as Record<string, unknown>) ?? {}), scene };
    await workbenchApi.updateTemplate(detail.value.id, { name: templateName.value, meta });
    message.success("场景已保存到模板 meta.scene");
    await loadTemplates();
    await selectTemplate(detail.value);
  } catch (e) {
    message.error((e as Error).message);
  }
}

async function validateScene() {
  if (!detail.value) return;
  const scene = parseJson(sceneText.value, "场景");
  if (scene === undefined) return;
  try {
    const res = await previewApi.validateScene(detail.value.snapshot, scene);
    message.success(`场景校验通过（${res.seat_count} 人）`);
  } catch (e) {
    message.error((e as Error).message);
  }
}

async function grabRoomTemplate() {
  if (!grabRoom.value.trim()) {
    message.warning("请输入房间 id");
    return;
  }
  try {
    const res = await workbenchApi.fromRoom({
      room_id: grabRoom.value.trim(),
      name: templateName.value || undefined,
    });
    message.success(`已抓取模板 #${res.row.id}`);
    await loadTemplates();
  } catch (e) {
    message.error((e as Error).message);
  }
}

async function importTemplate() {
  const snapshot = parseJson(importText.value, "snapshot");
  if (snapshot === undefined) return;
  if (!templateName.value.trim()) {
    message.warning("请填写模板名称");
    return;
  }
  try {
    const res = await workbenchApi.createTemplate({
      game_id: "sanguosha",
      name: templateName.value.trim(),
      source: "manual",
      snapshot,
    });
    message.success(`模板已保存 #${res.row.id}`);
    importText.value = "";
    await loadTemplates();
  } catch (e) {
    message.error((e as Error).message);
  }
}

async function duplicateTemplate(row: TemplateRow) {
  try {
    await workbenchApi.duplicateTemplate(row.id);
    message.success("已复制模板");
    await loadTemplates();
  } catch (e) {
    message.error((e as Error).message);
  }
}

function deleteTemplate(row: TemplateRow) {
  dialog.warning({
    title: "删除模板",
    content: `确定删除 #${row.id} ${row.name}？`,
    positiveText: "删除",
    negativeText: "取消",
    onPositiveClick: async () => {
      try {
        await workbenchApi.deleteTemplate(row.id);
        if (selectedId.value === row.id) {
          selectedId.value = null;
          detail.value = null;
        }
        message.success("已删除");
        await loadTemplates();
      } catch (e) {
        message.error((e as Error).message);
      }
    },
  });
}

function exportTemplate() {
  if (!detail.value) return;
  const payload = {
    game_id: detail.value.game_id,
    name: detail.value.name,
    source: detail.value.source,
    snapshot: detail.value.snapshot,
    meta: detail.value.meta,
  };
  downloadJson(`${detail.value.name || "template"}.json`, payload);
}

function downloadJson(name: string, value: unknown) {
  const blob = new Blob([JSON.stringify(value, null, 2)], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.click();
  URL.revokeObjectURL(url);
}

// 创建对局（沙盒开局）
const KINGDOM_LABEL: Record<string, string> = {
  wei: "魏",
  shu: "蜀",
  wu: "吴",
  qun: "群",
  jin: "晋",
  god: "神",
};
const IDENTITY_OPTIONS = [
  { label: "主公", value: "lord" },
  { label: "忠臣", value: "loyalist" },
  { label: "反贼", value: "rebel" },
  // v2 的引擎 code 是 `traitor`；中台/统计旧数据用 `spy`（引擎侧已做别名，两者都可用）。
  { label: "内奸", value: "traitor" },
];
interface SandboxSeatForm {
  general: string | null;
  identity: string | null;
}
interface SandboxForm {
  players: number;
  seed: number | null;
  saveTemplate: boolean;
  name: string;
  seats: SandboxSeatForm[];
}
function makeSeats(n: number): SandboxSeatForm[] {
  return Array.from({ length: n }, () => ({ general: null, identity: null }));
}
const createOpen = ref(false);
const createLoading = ref(false);
const sandboxMeta = ref<SandboxMeta | null>(null);
const createForm = ref<SandboxForm>({
  players: 5,
  seed: null,
  saveTemplate: false,
  name: "",
  seats: makeSeats(5),
});
const playerOptions = [5, 6, 7, 8].map((n) => ({ label: `${n} 人`, value: n }));

async function openCreate() {
  if (!sandboxMeta.value) {
    createLoading.value = true;
    try {
      sandboxMeta.value = await previewApi.meta();
    } catch (e) {
      message.error((e as Error).message);
      return;
    } finally {
      createLoading.value = false;
    }
  }
  createForm.value = {
    players: 5,
    seed: null,
    saveTemplate: false,
    name: "",
    seats: makeSeats(5),
  };
  createOpen.value = true;
}

function onPlayersChange(n: number) {
  const seats = createForm.value.seats.slice(0, n);
  while (seats.length < n) seats.push({ general: null, identity: null });
  createForm.value.seats = seats;
}

function generalOptions(index: number) {
  const used = new Set(
    createForm.value.seats
      .map((s, i) => (i === index ? null : s.general))
      .filter((v): v is string => Boolean(v)),
  );
  return (sandboxMeta.value?.generals ?? []).map((g) => ({
    label: `${g.name} · ${KINGDOM_LABEL[g.kingdom] ?? g.kingdom}`,
    value: g.code,
    name: g.name,
    disabled: used.has(g.code),
  }));
}

function filterGeneral(pattern: string, option: unknown): boolean {
  const o = option as { name?: string; label?: string };
  const name = String(o.name ?? o.label ?? "").toLowerCase();
  return name.includes(pattern.trim().toLowerCase());
}

function identityOptions(index: number) {
  const total = sandboxMeta.value?.identities[String(createForm.value.players)] ?? {};
  const used: Record<string, number> = {};
  createForm.value.seats.forEach((s, i) => {
    if (i !== index && s.identity) used[s.identity] = (used[s.identity] ?? 0) + 1;
  });
  const current = createForm.value.seats[index]?.identity;
  return IDENTITY_OPTIONS.map(({ label, value }) => {
    // 兼容：引擎 meta 同时给出 `traitor` 与 `spy` 别名。
    const alias = value === "traitor" ? "spy" : value;
    const totalFor = (total[value] ?? total[alias] ?? 0) as number;
    const usedFor = (used[value] ?? used[alias] ?? 0) as number;
    const remain = totalFor - usedFor;
    return {
      label: `${label}（剩余 ${Math.max(0, remain)}）`,
      value,
      disabled: remain <= 0 && current !== value,
    };
  });
}

async function submitCreate() {
  if (!sandboxMeta.value) return;
  const players = createForm.value.players;
  if (players < 5 || players > 8) {
    message.warning("玩家人数需在 5–8 之间");
    return;
  }
  const seats = createForm.value.seats.slice(0, players).map((s, i) => ({
    index: i,
    ...(s.general ? { general: s.general } : {}),
    ...(s.identity ? { identity: s.identity } : {}),
  }));
  previewBusy.value = true;
  try {
    const res = await previewApi.sandbox({
      players,
      seed: createForm.value.seed ?? undefined,
      seats,
      save_template: createForm.value.saveTemplate,
      name: createForm.value.name.trim() || undefined,
    });
    previewId.value = res.preview_id;
    const list = extractSeats(res);
    previewViewer.value = list[0]?.player_id ?? "";
    previewState.value = res;
    lastTrace.value = null;
    createOpen.value = false;
    tab.value = "preview";
    if (createForm.value.saveTemplate) await loadTemplates();
    message.success(`创建成功（seed=${res.seed}）`);
  } catch (e) {
    message.error((e as Error).message);
  } finally {
    previewBusy.value = false;
  }
}

// 预览
const WEB_URL =
  (import.meta.env.VITE_WEB_URL as string | undefined) ??
  (import.meta.env.DEV ? "http://127.0.0.1:18080" : "");
const previewId = ref("");
const previewState = ref<PreviewState | null>(null);
const previewViewer = ref("");
const previewBusy = ref(false);
const lastTrace = ref<PreviewStep | null>(null);

function extractSeats(state: PreviewState | null): PreviewSeat[] {
  const s = state?.seats;
  if (Array.isArray(s)) return s;
  if (s && Array.isArray((s as { seats?: PreviewSeat[] }).seats)) {
    return (s as { seats: PreviewSeat[] }).seats;
  }
  return [];
}

const seats = computed<PreviewSeat[]>(() => extractSeats(previewState.value));
const viewerOptions = computed(() =>
  seats.value.map((s) => ({
    label: `#${s.index + 1} ${s.general_name || s.player_id}`,
    value: s.player_id,
  })),
);
const iframeSrc = computed(() => {
  if (!previewId.value) return "";
  const q = new URLSearchParams({ room: previewId.value, viewer: previewViewer.value });
  return `${WEB_URL}/tabletop_game/sanguosha/preview?${q.toString()}`;
});

async function createPreview() {
  if (!detail.value) {
    message.warning("请先选择模板");
    return;
  }
  const scene = parseJson(sceneText.value, "场景");
  if (scene === undefined) return;
  previewBusy.value = true;
  try {
    const res = await previewApi.create({ template_id: detail.value.id, scene });
    previewId.value = res.preview_id;
    const list = extractSeats(res);
    previewViewer.value = list[0]?.player_id ?? "";
    previewState.value = res;
    lastTrace.value = null;
    tab.value = "preview";
    message.success(`预览会话已创建：${res.preview_id}`);
  } catch (e) {
    message.error((e as Error).message);
  } finally {
    previewBusy.value = false;
  }
}

async function refreshPreview(raw = true) {
  if (!previewId.value) return;
  try {
    previewState.value = await previewApi.get(previewId.value, previewViewer.value, raw);
  } catch (e) {
    message.error((e as Error).message);
  }
}

async function aiStep() {
  if (!previewId.value) return;
  try {
    const res = await previewApi.drive(previewId.value, {
      max_steps: 1,
      actor: previewViewer.value || undefined,
      viewer: previewViewer.value || undefined,
    });
    lastTrace.value = res.trace ?? null;
    message.success(`AI 单步：${res.trace?.action ?? "-"}（${res.stop ?? ""}）`);
    await refreshPreview(false);
  } catch (e) {
    message.error((e as Error).message);
  }
}

async function aiDrive() {
  if (!previewId.value) return;
  try {
    const res = await previewApi.drive(previewId.value, {
      max_steps: 50,
      viewer: previewViewer.value || undefined,
    });
    lastTrace.value = res.trace ?? null;
    message.success(`AI 推进 ${res.steps} 步（停止：${res.stop ?? "-"}）`);
    await refreshPreview(false);
  } catch (e) {
    message.error((e as Error).message);
  }
}

async function resetPreview() {
  if (!previewId.value) return;
  try {
    await previewApi.reset(previewId.value);
    lastTrace.value = null;
    message.success("已回到模板基线");
    await refreshPreview(false);
  } catch (e) {
    message.error((e as Error).message);
  }
}

function destroyPreview() {
  if (!previewId.value) return;
  dialog.warning({
    title: "销毁预览会话",
    content: "确定销毁当前预览？内存态会丢失（不影响模板/对局）。",
    positiveText: "销毁",
    negativeText: "取消",
    onPositiveClick: async () => {
      try {
        await previewApi.remove(previewId.value);
        previewId.value = "";
        previewState.value = null;
        lastTrace.value = null;
        message.success("预览会话已销毁");
      } catch (e) {
        message.error((e as Error).message);
      }
    },
  });
}

async function copyPreviewLink() {
  if (!iframeSrc.value) return;
  try {
    await navigator.clipboard.writeText(iframeSrc.value);
    message.success("预览链接已复制");
  } catch {
    message.warning(iframeSrc.value);
  }
}

const previewRows = computed<PreviewStep[]>(() => (previewState.value?.steps ?? []).slice(-10).reverse());

// 决策日志
const logRoom = ref("");
const logSince = ref<number | null>(null);
const logLimit = ref<number | null>(100);
const logRows = ref<DecisionLogRow[]>([]);
const logLoading = ref(false);

async function queryLogs() {
  if (!logRoom.value.trim()) {
    message.warning("请输入 room_id");
    return;
  }
  logLoading.value = true;
  try {
    const res = await logApi.list({
      room_id: logRoom.value.trim(),
      since_seq: logSince.value ?? undefined,
      limit: logLimit.value ?? 100,
    });
    logRows.value = res.rows;
    message.success(`查到 ${res.rows.length} 条决策日志`);
  } catch (e) {
    message.error((e as Error).message);
  } finally {
    logLoading.value = false;
  }
}

function exportLogs() {
  if (logRows.value.length === 0) return;
  const lines = logRows.value.map((r) =>
    JSON.stringify({
      id: r.id,
      seq: r.seq,
      player_hash: r.player_id,
      source: r.source,
      chosen: r.chosen,
      confidence: r.confidence,
      latency_ms: r.latency_ms,
      reason: r.reason,
      state_digest: r.state_digest,
      created_at: r.created_at,
    }),
  );
  const blob = new Blob([lines.join("\n")], { type: "application/x-ndjson" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = `decision-log-${logRoom.value || "room"}.jsonl`;
  a.click();
  URL.revokeObjectURL(url);
}

onMounted(loadTemplates);
</script>

<template>
  <div class="workbench">
    <h2 style="margin-top: 0">工作台 <span class="muted">模板 / 预览 / AI 调试 / 决策日志（P5 基座）</span></h2>
    <n-tabs v-model:value="tab" type="line" animated>
      <n-tab-pane name="templates" tab="模板">
        <div class="grid">
          <n-card title="模板列表" size="small">
            <n-space vertical>
              <n-space>
                <n-input v-model:value="grabRoom" placeholder="从对局房间抓取：room_id" style="width: 220px" />
                <n-button @click="grabRoomTemplate">抓取当前对局</n-button>
                <n-button type="primary" @click="openCreate">创建对局</n-button>
                <n-button @click="loadTemplates" :loading="loading">刷新</n-button>
              </n-space>
              <table class="wb-table">
                <thead>
                  <tr><th>#</th><th>名称</th><th>来源</th><th>更新时间</th><th>操作</th></tr>
                </thead>
                <tbody>
                  <tr
                    v-for="row in templates"
                    :key="row.id"
                    :class="{ selected: row.id === selectedId }"
                    @click="selectTemplate(row)"
                  >
                    <td>{{ row.id }}</td>
                    <td>{{ row.name }}</td>
                    <td class="mono small">{{ row.source }}</td>
                    <td class="mono small">{{ row.updated_at }}</td>
                    <td>
                      <n-space :size="4">
                        <n-button size="tiny" @click.stop="duplicateTemplate(row)">复制</n-button>
                        <n-button size="tiny" type="error" ghost @click.stop="deleteTemplate(row)">删除</n-button>
                      </n-space>
                    </td>
                  </tr>
                  <tr v-if="templates.length === 0"><td colspan="5" class="muted">暂无模板</td></tr>
                </tbody>
              </table>
            </n-space>
          </n-card>

          <n-card title="模板详情 / 场景注入" size="small">
            <template v-if="detail">
              <n-space vertical>
                <n-space>
                  <n-input v-model:value="templateName" placeholder="模板名称" style="width: 280px" />
                  <n-button @click="saveScene">保存名称+场景</n-button>
                  <n-button @click="validateScene">校验场景</n-button>
                  <n-button type="primary" @click="createPreview">创建预览</n-button>
                  <n-button @click="exportTemplate">导出 JSON</n-button>
                </n-space>
                <n-tabs v-model:value="detailTab" type="segment" size="small">
                  <n-tab-pane name="scene" tab="场景 SceneSpec">
                    <n-input
                      v-model:value="sceneText"
                      type="textarea"
                      :autosize="{ minRows: 12, maxRows: 26 }"
                      placeholder='{"phase":"play","turn":0,"round":1,"seats":[{"index":0,"general":"sgs_hero_1","hand":["sha"]}]}'
                    />
                    <p class="muted small">
                      卡牌 token：牌实例 id（c12）或定义码（sha）；定义码只能从摸牌堆/弃牌堆/移出区取，不能重复。
                    </p>
                  </n-tab-pane>
                  <n-tab-pane name="snapshot" tab="快照 JSON">
                    <pre class="code">{{ JSON.stringify(detail.snapshot, null, 2) }}</pre>
                  </n-tab-pane>
                  <n-tab-pane name="meta" tab="meta">
                    <pre class="code">{{ JSON.stringify(detail.meta, null, 2) }}</pre>
                  </n-tab-pane>
                </n-tabs>
              </n-space>
            </template>
            <template v-else>
              <p class="muted">选择左侧模板查看/编辑。</p>
              <n-space vertical>
                <n-input v-model:value="templateName" placeholder="新模板名称" />
                <n-input
                  v-model:value="importText"
                  type="textarea"
                  :autosize="{ minRows: 8, maxRows: 18 }"
                  placeholder="粘贴引擎完整 GameState JSON（含 data 私有状态）"
                />
                <n-button @click="importTemplate">导入并保存模板</n-button>
              </n-space>
            </template>
          </n-card>
        </div>
      </n-tab-pane>

      <n-tab-pane name="preview" tab="预览">
        <n-card size="small">
          <n-space vertical>
            <n-space align="center">
              <n-tag type="success">预览</n-tag>
              <span class="mono small">{{ previewId || "（未创建）" }}</span>
              <n-select
                v-if="viewerOptions.length"
                v-model:value="previewViewer"
                :options="viewerOptions"
                style="width: 220px"
                @update:value="refreshPreview(false)"
              />
              <n-button @click="refreshPreview(true)">刷新</n-button>
              <n-button @click="aiStep" :disabled="!previewId">AI 单步</n-button>
              <n-button @click="aiDrive" :disabled="!previewId">AI 连推</n-button>
              <n-button @click="resetPreview" :disabled="!previewId">重置</n-button>
              <n-button type="error" ghost @click="destroyPreview" :disabled="!previewId">销毁</n-button>
              <n-button @click="copyPreviewLink" :disabled="!previewId">复制链接</n-button>
            </n-space>
            <p v-if="previewState" class="muted small">
              {{ previewState.summary }} · 已选视角：{{ previewViewer || "-" }} · 完成：{{ previewState.finished ? "是" : "否" }}
            </p>
            <iframe v-if="previewId" :src="iframeSrc" class="preview-frame" title="三国杀预览" />
            <p v-else class="muted">先在「模板」TAB 选择模板并点「创建预览」。</p>
          </n-space>
        </n-card>
      </n-tab-pane>

      <n-tab-pane name="ai" tab="AI 调试">
        <div class="grid">
          <n-card title="状态（raw）" size="small">
            <n-space vertical>
              <n-space>
                <n-button @click="refreshPreview(true)" :disabled="!previewId">刷新 raw</n-button>
                <n-button @click="aiStep" :disabled="!previewId">AI 单步</n-button>
                <n-button @click="aiDrive" :disabled="!previewId">AI 连推 50</n-button>
              </n-space>
              <p class="muted small">摘要：{{ previewState?.summary ?? "-" }}</p>
              <pre class="code">{{ JSON.stringify(previewState?.raw ?? null, null, 2) }}</pre>
            </n-space>
          </n-card>
          <n-card title="当前视角合法动作 / 最近 AI 轨迹" size="small">
            <n-space vertical>
              <div>
                <strong>合法动作（{{ previewState?.actions?.length ?? 0 }}）</strong>
                <ul class="actions">
                  <li v-for="a in previewState?.actions ?? []" :key="a.id">
                    <span class="mono">{{ a.id }}</span> · {{ a.description }}
                  </li>
                </ul>
              </div>
              <div v-if="lastTrace">
                <strong>最近一次 AI</strong>
                <p class="mono small">
                  {{ lastTrace.actor }} → {{ lastTrace.action }} · score={{ lastTrace.score ?? "-" }} ·
                  priority={{ lastTrace.priority ?? "-" }} · {{ lastTrace.reason ?? "-" }} ·
                  {{ lastTrace.latency_ms ?? "-" }}ms
                </p>
              </div>
              <table class="wb-table">
                <thead><tr><th>seq</th><th>actor</th><th>action</th><th>score</th><th>reason</th><th>ms</th></tr></thead>
                <tbody>
                  <tr v-for="s in previewRows" :key="s.seq">
                    <td>{{ s.seq }}</td>
                    <td class="mono small">{{ s.actor }}</td>
                    <td class="mono small">{{ s.action }}</td>
                    <td>{{ s.score ?? "-" }}</td>
                    <td class="small">{{ s.reason ?? s.source }}</td>
                    <td>{{ s.latency_ms ?? "-" }}</td>
                  </tr>
                  <tr v-if="previewRows.length === 0"><td colspan="6" class="muted">暂无步骤</td></tr>
                </tbody>
              </table>
            </n-space>
          </n-card>
        </div>
      </n-tab-pane>

      <n-tab-pane name="logs" tab="决策日志">
        <n-card size="small">
          <n-space vertical>
            <n-space align="center">
              <n-input v-model:value="logRoom" placeholder="room_id" style="width: 320px" />
              <n-input-number v-model:value="logSince" placeholder="since_seq" :min="0" style="width: 130px" />
              <n-input-number v-model:value="logLimit" placeholder="limit" :min="1" :max="500" style="width: 110px" />
              <n-button @click="queryLogs" :loading="logLoading">查询</n-button>
              <n-button @click="exportLogs" :disabled="logRows.length === 0">导出 JSONL</n-button>
            </n-space>
            <p class="muted small">player_id 已按 sha256 前 16 位哈希（与 sgs:export-decisions 口径一致）。</p>
            <table class="wb-table">
              <thead><tr><th>seq</th><th>player_hash</th><th>source</th><th>chosen</th><th>score</th><th>reason</th><th>time</th></tr></thead>
              <tbody>
                <tr v-for="r in logRows" :key="r.id">
                  <td>{{ r.seq }}</td>
                  <td class="mono small">{{ r.player_id }}</td>
                  <td>{{ r.source }}</td>
                  <td class="mono small">{{ r.chosen ?? "-" }}</td>
                  <td>{{ r.confidence ?? "-" }}</td>
                  <td class="small">{{ r.reason ?? "-" }}</td>
                  <td class="mono small">{{ r.created_at }}</td>
                </tr>
                <tr v-if="logRows.length === 0"><td colspan="7" class="muted">暂无数据</td></tr>
              </tbody>
            </table>
          </n-space>
        </n-card>
      </n-tab-pane>
    </n-tabs>

    <n-drawer v-model:show="createOpen" placement="right" :width="640">
      <n-drawer-content title="创建对局（模拟开局 · 全 bot）" closable>
        <n-form label-placement="left" label-width="92" require-mark-placement="right-hanging">
          <n-form-item label="玩家数" required>
            <n-select
              v-model:value="createForm.players"
              :options="playerOptions"
              style="width: 180px"
              @update:value="onPlayersChange"
            />
          </n-form-item>
          <n-form-item label="随机种子">
            <n-input-number
              v-model:value="createForm.seed"
              :min="0"
              clearable
              placeholder="留空自动随机"
              style="width: 220px"
            />
          </n-form-item>
          <n-divider title-placement="left">座位（#1–#N；武将/身份不选即随机）</n-divider>
          <div v-for="(seat, i) in createForm.seats" :key="i" class="seat-row">
            <span class="seat-no">#{{ i + 1 }}</span>
            <n-select
              v-model:value="seat.general"
              :options="generalOptions(i)"
              filterable
              clearable
              :filter="filterGeneral"
              placeholder="随机武将（可搜索）"
              style="flex: 1"
            />
            <n-select
              v-model:value="seat.identity"
              :options="identityOptions(i)"
              clearable
              placeholder="随机身份"
              style="width: 170px"
            />
          </div>
          <n-form-item label="保存模板">
            <n-checkbox v-model:checked="createForm.saveTemplate">同时保存为模板（可在列表中复用）</n-checkbox>
          </n-form-item>
          <n-form-item v-if="createForm.saveTemplate" label="模板名称">
            <n-input v-model:value="createForm.name" placeholder="留空自动命名" />
          </n-form-item>
        </n-form>
        <template #footer>
          <n-space justify="end">
            <n-button @click="createOpen = false">取消</n-button>
            <n-button type="primary" :loading="previewBusy" @click="submitCreate">创建并预览</n-button>
          </n-space>
        </template>
      </n-drawer-content>
    </n-drawer>
  </div>
</template>

<style scoped>
.workbench {
  min-height: 100%;
}
.muted {
  color: #6e7781;
}
.small {
  font-size: 12px;
}
.mono {
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
}
.grid {
  display: grid;
  grid-template-columns: minmax(360px, 1fr) minmax(420px, 1.4fr);
  gap: 14px;
  align-items: start;
}
@media (max-width: 1100px) {
  .grid {
    grid-template-columns: 1fr;
  }
}
.wb-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 13px;
  color: #1f2328;
}
.wb-table th,
.wb-table td {
  border-bottom: 1px solid #30363d;
  padding: 5px 6px;
  text-align: left;
  vertical-align: top;
}
.wb-table tbody tr {
  cursor: pointer;
}
.wb-table tbody tr:hover {
  background: #f3f6fa;
}
/* 选中行：浅底深字（历史深底 + 深色文字在浅色主题下不可读） */
.wb-table tbody tr.selected,
.wb-table tbody tr.selected:hover {
  background: #e3efff;
  color: #111418;
}
.code {
  max-height: 52vh;
  overflow: auto;
  background: #0d1117;
  color: #c9d1d9;
  padding: 10px;
  border-radius: 8px;
  font-size: 12px;
  line-height: 1.45;
}
.preview-frame {
  width: 100%;
  height: 72vh;
  border: 1px solid #30363d;
  border-radius: 10px;
  background: #0b0f14;
}
.seat-row {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
}
.seat-no {
  width: 40px;
  color: #6e7781;
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 12px;
}
.actions {
  margin: 6px 0;
  padding-left: 18px;
  font-size: 12px;
  max-height: 180px;
  overflow: auto;
}
</style>
