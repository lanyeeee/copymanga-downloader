<script setup lang="ts">
import {onMounted, ref, watch} from "vue";
import {Comic, commands, Config, UserProfileRespData} from "./bindings.ts";
import {useMessage, useNotification} from "naive-ui";
import LoginDialog from "./components/LoginDialog.vue";
import SearchPane from "./components/SearchPane.vue";
import ChapterPane from "./components/ChapterPane.vue";

const message = useMessage();
const notification = useNotification();

const config = ref<Config>();
const userProfile = ref<UserProfileRespData>();
const loginDialogShowing = ref<boolean>(false);
const currentTabName = ref<"search" | "chapter">("search");
const selectedComic = ref<Comic>();

watch(config, async () => {
  if (config.value === undefined) {
    return;
  }
  await commands.saveConfig(config.value);
  message.success("保存配置成功");
}, {deep: true});

watch(() => config.value?.token, async () => {
  if (config.value === undefined || config.value.token === "") {
    return;
  }
  const result = await commands.getUserProfile();
  if (result.status === "error") {
    notification.error({title: "获取用户信息失败", description: result.error});
    userProfile.value = undefined;
    return;
  }
  userProfile.value = result.data;
  message.success("获取用户信息成功");
});

onMounted(async () => {
  // 屏蔽浏览器右键菜单
  document.oncontextmenu = (event) => {
    event.preventDefault();
  };
  // 获取配置
  config.value = await commands.getConfig();
});

async function test() {
  const result = await commands.getComic("modujingbingdenuli");
  console.log(result);
}

</script>

<template>
  <div v-if="config!==undefined" class="h-screen flex flex-col">
    <div class="flex">
      <n-input v-model:value="config.token" placeholder="" clearable>
        <template #prefix>
          Token：
        </template>
      </n-input>
      <n-button type="primary" @click="loginDialogShowing=true">账号登录</n-button>
      <n-button @click="test">测试用</n-button>
      <div v-if="userProfile!==undefined" class="flex flex-justify-end">
        <n-avatar round :size="32" :src="userProfile.avatar"/>
        <span class="whitespace-nowrap">{{ userProfile.nickname }}</span>
      </div>
    </div>
    <div class="flex flex-1 overflow-hidden">
      <n-tabs class="h-full" v-model:value="currentTabName" type="line" size="small">
        <n-tab-pane class="h-full overflow-auto p-0!" name="search" tab="漫画搜索" display-directive="show:lazy">
          <search-pane v-model:selected-comic="selectedComic" v-model:current-tab-name="currentTabName"/>
        </n-tab-pane>
        <n-tab-pane class="h-full overflow-auto p-0!" name="chapter" tab="章节详情" display-directive="show:lazy">
          <chapter-pane v-model:selected-comic="selectedComic"/>
        </n-tab-pane>
      </n-tabs>
    </div>
    <n-modal v-model:show="loginDialogShowing">
      <login-dialog v-model:showing="loginDialogShowing" v-model:config="config"/>
    </n-modal>
  </div>
</template>
