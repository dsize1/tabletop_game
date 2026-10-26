<script setup lang="ts">
import { computed, ref } from "vue";
import { useRoute, useRouter } from "vue-router";
import { NConfigProvider, NLayout, NLayoutSider, NLayoutHeader, NLayoutContent, NMenu, NInput, NButton, NSpace, NMessageProvider, NDialogProvider } from "naive-ui";
import { getAdminToken, setAdminToken } from "./api";

const route = useRoute();
const router = useRouter();
const token = ref(getAdminToken());
const tokenSaved = ref(false);

const menuOptions = [
  { label: "概览", key: "/" },
  { label: "道具包", key: "/packs" },
  { label: "身份", key: "/identities" },
  { label: "身份配比", key: "/identity-setup" },
  { label: "牌定义", key: "/card-defs" },
  { label: "物理牌", key: "/cards" },
  { label: "定义↔物理牌", key: "/card-relation" },
  { label: "武将", key: "/generals" },
  { label: "技能", key: "/skills" },
  { label: "武将技能关联", key: "/general-skills" },
  { label: "数据导入", key: "/import" },
];

const activeKey = computed(() => route.path);

function go(key: string) {
  router.push(key);
}

function saveToken() {
  setAdminToken(token.value.trim());
  tokenSaved.value = true;
  setTimeout(() => (tokenSaved.value = false), 1500);
}
</script>

<template>
  <n-config-provider>
    <n-message-provider>
      <n-dialog-provider>
        <n-layout style="height: 100vh">
          <n-layout-header bordered style="padding: 12px 20px; display: flex; align-items: center; gap: 16px">
            <strong style="font-size: 16px">桌游道具中台</strong>
            <span style="color: #888; font-size: 12px">三国杀 P1</span>
            <div style="flex: 1" />
            <n-space align="center">
              <span style="font-size: 12px; color: #888">Admin Token</span>
              <n-input v-model:value="token" type="password" placeholder="PROPS_ADMIN_TOKEN（可空）" style="width: 260px" />
              <n-button size="small" @click="saveToken">{{ tokenSaved ? "已保存" : "保存" }}</n-button>
            </n-space>
          </n-layout-header>
          <n-layout has-sider style="height: calc(100vh - 57px)">
            <n-layout-sider width="200" bordered content-style="padding: 8px">
              <n-menu :options="menuOptions" :value="activeKey" @update:value="go" />
            </n-layout-sider>
            <n-layout-content content-style="padding: 20px">
              <router-view />
            </n-layout-content>
          </n-layout>
        </n-layout>
      </n-dialog-provider>
    </n-message-provider>
  </n-config-provider>
</template>
