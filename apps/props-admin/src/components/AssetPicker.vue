<script setup lang="ts">
import { computed, ref, watch } from "vue";
import {
  NButton,
  NEmpty,
  NInput,
  NModal,
  NSpace,
  NSpin,
  useMessage,
} from "naive-ui";
import { api, type PropAsset } from "../api";
import AssetCropDialog from "./AssetCropDialog.vue";

const props = defineProps<{
  show: boolean;
  gameId?: string;
  kind: string;
  title?: string;
}>();
const emit = defineEmits<{
  (e: "update:show", v: boolean): void;
  (e: "picked", asset: PropAsset): void;
}>();

const message = useMessage();
const rows = ref<PropAsset[]>([]);
const loading = ref(false);
const search = ref("");
const showCrop = ref(false);
const showModel = computed({
  get: () => props.show,
  set: (v: boolean) => emit("update:show", v),
});

async function load() {
  loading.value = true;
  try {
    const res = await api.listAssets({ gameId: props.gameId, kind: props.kind, q: search.value.trim() || undefined });
    rows.value = res.rows;
  } catch (e) {
    message.error((e as Error).message);
  } finally {
    loading.value = false;
  }
}

watch(
  () => props.show,
  (v) => {
    if (v) void load();
  },
);

watch(search, () => {
  if (props.show) void load();
});

function pick(asset: PropAsset) {
  emit("picked", asset);
  showModel.value = false;
}

function onUploaded(asset: PropAsset) {
  rows.value = [asset, ...rows.value.filter((r) => r.id !== asset.id)];
  pick(asset);
  void load();
}
</script>

<template>
  <n-modal v-model:show="showModel" preset="card" :title="title ?? '选择素材'" style="width: 860px">
    <n-space vertical :size="12">
      <n-space>
        <n-input
          v-model:value="search"
          placeholder="搜索路径 / 原始文件名"
          clearable
          style="width: 260px"
        />
        <n-button @click="load">刷新</n-button>
        <n-button type="primary" @click="showCrop = true">上传新素材</n-button>
      </n-space>
      <n-spin :show="loading">
        <div v-if="rows.length === 0" style="padding: 24px 0">
          <n-empty description="暂无素材：先上传一张" />
        </div>
        <div
          v-else
          style="display: grid; grid-template-columns: repeat(auto-fill, minmax(120px, 1fr)); gap: 10px; max-height: 60vh; overflow: auto"
        >
          <button
            v-for="a in rows"
            :key="a.id"
            type="button"
            style="border: 1px solid #333; border-radius: 8px; padding: 6px; background: #1a1d23; cursor: pointer; display: flex; flex-direction: column; gap: 4px; align-items: center"
            @click="pick(a)"
          >
            <img
              :src="a.thumb_url"
              :alt="a.path"
              loading="lazy"
              style="width: 96px; height: 135px; object-fit: cover; border-radius: 6px"
            />
            <span style="font-size: 11px; color: #aaa">
              #{{ a.id }} · {{ a.width ?? "?" }}×{{ a.height ?? "?" }}
            </span>
          </button>
        </div>
      </n-spin>
    </n-space>
    <asset-crop-dialog
      v-model:show="showCrop"
      :game-id="gameId"
      :kind="kind"
      @uploaded="onUploaded"
    />
  </n-modal>
</template>
