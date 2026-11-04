<script setup lang="ts">
import { onMounted, ref } from "vue";
import {
  NButton,
  NEmpty,
  NInput,
  NPopconfirm,
  NSelect,
  NSpace,
  NSpin,
  NTag,
  useMessage,
} from "naive-ui";
import { api, type PropAsset } from "../api";
import AssetCropDialog from "../components/AssetCropDialog.vue";

const message = useMessage();
const rows = ref<PropAsset[]>([]);
const loading = ref(false);
const search = ref("");
const kind = ref<string | null>(null);
const showCrop = ref(false);
const uploadKind = ref("general");

const kindOptions = [
  { label: "全部类型", value: "" },
  { label: "武将卡面", value: "general" },
  { label: "身份图", value: "identity" },
  { label: "卡牌图", value: "card" },
];

const uploadKindOptions = [
  { label: "武将卡面（512×720）", value: "general" },
  { label: "身份图（384×512）", value: "identity" },
  { label: "卡牌图（384×512）", value: "card" },
];

async function load() {
  loading.value = true;
  try {
    const res = await api.listAssets({
      kind: kind.value || undefined,
      q: search.value.trim() || undefined,
      limit: 1000,
    });
    rows.value = res.rows;
  } catch (e) {
    message.error((e as Error).message);
  } finally {
    loading.value = false;
  }
}

function openUpload() {
  showCrop.value = true;
}

function onUploaded(asset: PropAsset) {
  rows.value = [asset, ...rows.value.filter((r) => r.id !== asset.id)];
  void load();
}

async function remove(asset: PropAsset) {
  try {
    await api.deleteAsset(asset.id);
    message.success(`已删除素材 #${asset.id}`);
    await load();
  } catch (e) {
    message.error((e as Error).message);
  }
}

async function copyPath(asset: PropAsset) {
  try {
    await navigator.clipboard.writeText(asset.path);
    message.success("已复制素材路径");
  } catch {
    message.warning(asset.path);
  }
}

function preview(asset: PropAsset) {
  window.open(asset.url, "_blank", "noopener");
}

function kb(bytes: number | null | undefined): string {
  return bytes ? `${Math.round(bytes / 1024)} KB` : "-";
}

onMounted(load);
</script>

<template>
  <div>
    <div style="display: flex; align-items: center; gap: 12px; margin-bottom: 16px">
      <h2 style="margin: 0">素材</h2>
      <span style="color: #888; font-size: 12px">内容寻址 · /game-assets 同源静态</span>
      <div style="flex: 1" />
      <n-input
        v-model:value="search"
        placeholder="搜索路径 / 原始文件名"
        clearable
        style="width: 220px"
        @keyup.enter="load"
      />
      <n-select
        v-model:value="kind"
        :options="kindOptions"
        style="width: 150px"
        @update:value="load"
      />
      <n-button @click="load">刷新</n-button>
      <n-select v-model:value="uploadKind" :options="uploadKindOptions" style="width: 200px" />
      <n-button type="primary" @click="openUpload">上传素材</n-button>
    </div>

    <n-spin :show="loading">
      <div v-if="rows.length === 0" style="padding: 40px 0">
        <n-empty description="暂无素材" />
      </div>
      <div
        v-else
        style="display: grid; grid-template-columns: repeat(auto-fill, minmax(168px, 1fr)); gap: 14px"
      >
        <div
          v-for="a in rows"
          :key="a.id"
          style="border: 1px solid #2b3038; border-radius: 10px; padding: 8px; background: #191c22; display: flex; flex-direction: column; gap: 6px"
        >
          <img
            :src="a.thumb_url"
            :alt="a.path"
            loading="lazy"
            style="width: 100%; aspect-ratio: 512 / 720; object-fit: cover; border-radius: 8px; background: #111"
            @click="preview(a)"
          />
          <div style="display: flex; align-items: center; gap: 6px; flex-wrap: wrap">
            <n-tag size="tiny">#{{ a.id }}</n-tag>
            <n-tag size="tiny" type="info">{{ a.kind }}</n-tag>
            <span style="font-size: 11px; color: #888">{{ a.width }}×{{ a.height }}</span>
          </div>
          <div style="font-size: 11px; color: #888; word-break: break-all">{{ a.path }}</div>
          <div style="font-size: 11px; color: #888">
            {{ kb(a.bytes) }}
            <template v-if="a.thumb_path"> · 缩略图 ✓</template>
          </div>
          <n-space :size="4">
            <n-button size="tiny" @click="preview(a)">预览</n-button>
            <n-button size="tiny" @click="copyPath(a)">复制路径</n-button>
            <n-popconfirm @positive-click="remove(a)">
              <template #trigger>
                <n-button size="tiny" type="error">删除</n-button>
              </template>
              被业务行引用时会拒绝删除，先解除绑定。
            </n-popconfirm>
          </n-space>
        </div>
      </div>
    </n-spin>

    <asset-crop-dialog
      v-model:show="showCrop"
      :kind="uploadKind"
      @uploaded="onUploaded"
    />
  </div>
</template>
