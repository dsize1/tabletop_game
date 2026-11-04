<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { NAlert, NButton, NModal, NSlider, NSpace, NSpin, useMessage } from "naive-ui";
import { api, type PropAsset } from "../api";
import {
  buildAssetExport,
  clampOffset,
  drawPreview,
  loadImage,
  presetOf,
} from "../image";

const props = defineProps<{
  show: boolean;
  gameId?: string;
  kind: string;
}>();
const emit = defineEmits<{
  (e: "update:show", v: boolean): void;
  (e: "uploaded", asset: PropAsset): void;
}>();

const message = useMessage();
const file = ref<File | null>(null);
const img = ref<HTMLImageElement | null>(null);
const zoom = ref(1);
const offsetX = ref(0);
const offsetY = ref(0);
const busy = ref(false);
const error = ref("");
const canvas = ref<HTMLCanvasElement | null>(null);

const preset = computed(() => presetOf(props.kind));
const PREVIEW_W = 240;
const PREVIEW_H = computed(() =>
  Math.max(1, Math.round((PREVIEW_W * preset.value.h) / preset.value.w)),
);

const showModel = computed({
  get: () => props.show,
  set: (v: boolean) => emit("update:show", v),
});

function redraw() {
  const c = canvas.value;
  const im = img.value;
  if (!c || !im) return;
  drawPreview(c, im, PREVIEW_W, PREVIEW_H.value, zoom.value, offsetX.value, offsetY.value);
}

function resetView() {
  zoom.value = 1;
  offsetX.value = 0;
  offsetY.value = 0;
}

async function pickFile(e: Event) {
  const input = e.target as HTMLInputElement;
  const f = input.files?.[0];
  input.value = "";
  if (!f) return;
  if (f.size > 20 * 1024 * 1024) {
    error.value = "原图超过 20MB，请先压缩";
    return;
  }
  error.value = "";
  try {
    const im = await loadImage(f);
    file.value = f;
    img.value = im;
    resetView();
    await nextTick();
    redraw();
  } catch (err) {
    error.value = (err as Error).message;
  }
}

function onDown(e: PointerEvent) {
  if (!img.value) return;
  const c = e.currentTarget as HTMLCanvasElement;
  c.setPointerCapture(e.pointerId);
  const startX = e.clientX;
  const startY = e.clientY;
  const baseX = offsetX.value;
  const baseY = offsetY.value;
  const move = (ev: PointerEvent) => {
    const im = img.value;
    if (!im) return;
    const next = clampOffset(
      im.width,
      im.height,
      PREVIEW_W,
      PREVIEW_H.value,
      zoom.value,
      baseX + (ev.clientX - startX),
      baseY + (ev.clientY - startY),
    );
    offsetX.value = next.x;
    offsetY.value = next.y;
  };
  const up = () => {
    c.removeEventListener("pointermove", move);
    c.removeEventListener("pointerup", up);
    c.removeEventListener("pointercancel", up);
  };
  c.addEventListener("pointermove", move);
  c.addEventListener("pointerup", up);
  c.addEventListener("pointercancel", up);
}

watch(zoom, () => {
  const im = img.value;
  if (!im) return;
  const next = clampOffset(
    im.width,
    im.height,
    PREVIEW_W,
    PREVIEW_H.value,
    zoom.value,
    offsetX.value,
    offsetY.value,
  );
  offsetX.value = next.x;
  offsetY.value = next.y;
});
watch([zoom, offsetX, offsetY], redraw);

async function confirm() {
  const im = img.value;
  if (!im) return;
  busy.value = true;
  error.value = "";
  try {
    const out = await buildAssetExport(
      im,
      PREVIEW_W,
      PREVIEW_H.value,
      zoom.value,
      offsetX.value,
      offsetY.value,
      preset.value.w,
      preset.value.h,
    );
    const up = await api.uploadAsset({
      gameId: props.gameId,
      kind: props.kind,
      width: preset.value.w,
      height: preset.value.h,
      filename: file.value?.name,
      crop: out.cropJson,
      blob: out.full,
    });
    let asset = up.asset;
    try {
      const t = await api.uploadThumb(asset.id, out.thumb, out.thumbW, out.thumbH);
      asset = t.asset;
    } catch (e) {
      message.warning(`缩略图上传失败（列表将回退原图）：${(e as Error).message}`);
    }
    message.success("素材已上传");
    emit("uploaded", asset);
    showModel.value = false;
    file.value = null;
    img.value = null;
    resetView();
  } catch (e) {
    error.value = (e as Error).message;
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <n-modal
    v-model:show="showModel"
    preset="card"
    :title="`上传素材（${preset.label}）`"
    style="width: 520px"
  >
    <n-space vertical :size="12">
      <input type="file" accept="image/png,image/jpeg,image/webp" @change="pickFile" />
      <n-alert v-if="error" type="error" :show-icon="false">{{ error }}</n-alert>
      <n-spin :show="busy">
        <div
          v-if="img"
          style="display: flex; flex-direction: column; gap: 8px; align-items: center"
        >
          <canvas
            ref="canvas"
            :style="{
              width: `${PREVIEW_W}px`,
              height: `${PREVIEW_H}px`,
              borderRadius: '8px',
              border: '1px solid #3a3f4a',
              cursor: 'grab',
              touchAction: 'none',
              background: '#111',
            }"
            @pointerdown.prevent="onDown"
          />
          <div style="width: 100%; display: flex; align-items: center; gap: 12px">
            <span style="font-size: 12px; color: #888">缩放</span>
            <n-slider v-model:value="zoom" :min="1" :max="3" :step="0.01" style="flex: 1" />
            <n-button size="tiny" @click="resetView">重置</n-button>
          </div>
          <div style="font-size: 12px; color: #888">
            拖动图片调整位置 · 输出 {{ preset.w }}×{{ preset.h }} · 缩略图
            {{ Math.round(preset.w / 2) }}×{{ Math.round(preset.h / 2) }}
          </div>
        </div>
        <div v-else style="font-size: 12px; color: #888">先选择一张图片（png/jpeg/webp）</div>
      </n-spin>
    </n-space>
    <template #footer>
      <n-space justify="end">
        <n-button @click="showModel = false">取消</n-button>
        <n-button type="primary" :loading="busy" :disabled="!img" @click="confirm">
          上传
        </n-button>
      </n-space>
    </template>
  </n-modal>
</template>
