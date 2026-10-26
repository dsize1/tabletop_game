<script setup lang="ts">
import { ref } from "vue";
import { NAlert, NButton, NCard, NCode, NSpace, useMessage } from "naive-ui";
import { api } from "../api";

const message = useMessage();
const loading = ref(false);
const result = ref<string>("");
const seedLoading = ref(false);

async function seed() {
  seedLoading.value = true;
  result.value = "";
  try {
    const res = await api.seed();
    result.value = JSON.stringify(res, null, 2);
    message.success("初始化完成：身份/配比/包已写入");
  } catch (e) {
    message.error((e as Error).message);
  } finally {
    seedLoading.value = false;
  }
}

async function importHeroes() {
  loading.value = true;
  result.value = "";
  try {
    const res = await api.importHeroes();
    result.value = JSON.stringify(res, null, 2);
    message.success(`导入完成：武将 ${res.imported.generals}，技能 ${res.imported.skills}`);
  } catch (e) {
    message.error((e as Error).message);
  } finally {
    loading.value = false;
  }
}
</script>

<template>
  <div>
    <h2 style="margin-top: 0">数据导入</h2>
    <n-space vertical size="large">
      <n-card size="small" title="基础数据（身份 / 配比 / 包）">
        <p>写入 4 种身份、5–8 人身份配比（含 8 人双内）、武将包与标准/军争牌堆包。幂等，可重复执行。</p>
        <n-button type="primary" :loading="seedLoading" @click="seed">初始化基础数据</n-button>
      </n-card>
      <n-card size="small" title="官网武将（heroes.json）">
        <p>
          读取 <n-code code="docs/games/sanguosha/data/heroes.json" />，幂等导入武将、技能、武将技能关联。
          导入后状态为 <n-code code="draft" />，请到「武将」「技能」页人工复核后上线。
        </p>
        <n-button type="primary" :loading="loading" @click="importHeroes">导入武将</n-button>
      </n-card>

      <n-card size="small" title="牌表（人工）">
        <n-alert type="info">
          官网无公开结构化牌表。按项目裁定牌表人工录入：先建牌定义，再到「定义↔物理牌」逐张添加花色/点数。
          <n-code code="data/cards.json" /> / <n-code code="cards.md" /> 仅作参考，入库前需逐张核对。
        </n-alert>
      </n-card>

      <n-card v-if="result" size="small" title="导入结果">
        <pre style="white-space: pre-wrap">{{ result }}</pre>
      </n-card>
    </n-space>
  </div>
</template>
