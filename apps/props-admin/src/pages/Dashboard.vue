<script setup lang="ts">
import { onMounted, ref } from "vue";
import { NButton, NCard, NGrid, NGridItem, NStatistic, NSpace, NTag, useMessage } from "naive-ui";
import { api } from "../api";

const message = useMessage();
const summary = ref<Record<string, unknown>>({});
const loading = ref(false);

async function load() {
  loading.value = true;
  try {
    summary.value = await api.summary();
  } catch (e) {
    message.error((e as Error).message);
  } finally {
    loading.value = false;
  }
}

const cards = [
  { key: "prop_packs", label: "道具包" },
  { key: "sgs_identities", label: "身份" },
  { key: "sgs_card_defs", label: "牌定义" },
  { key: "sgs_cards", label: "物理牌" },
  { key: "sgs_generals", label: "武将" },
  { key: "sgs_skills", label: "技能" },
];

function statusText(v: unknown): string {
  return String(v ?? "-");
}

onMounted(load);
</script>

<template>
  <div>
    <h2 style="margin-top: 0">概览</h2>
    <p style="color: #888">道具中台 P1：元数据 / 生命周期 / 官网武将导入 / 定义↔物理牌关联。</p>
    <n-space style="margin-bottom: 16px">
      <n-button :loading="loading" @click="load">刷新</n-button>
      <n-button tag="a" href="/import" type="primary">数据导入</n-button>
      <n-button tag="a" href="/card-relation">定义↔物理牌</n-button>
    </n-space>
    <n-grid :cols="3" :x-gap="12" :y-gap="12">
      <n-grid-item v-for="c in cards" :key="c.key">
        <n-card size="small">
          <n-statistic :label="c.label" :value="Number(summary[c.key] ?? 0)" />
          <div v-if="summary[`${c.key}_status`]" style="margin-top: 8px">
            <n-tag
              v-for="(v, k) in (summary[`${c.key}_status`] as Record<string, number>)"
              :key="k"
              size="small"
              style="margin-right: 4px"
            >
              {{ statusText(k) }} {{ v }}
            </n-tag>
          </div>
        </n-card>
      </n-grid-item>
    </n-grid>
  </div>
</template>
