<script setup lang="ts">
import { onMounted, ref, watch } from 'vue'
import { Comic, commands, Config, UserProfileRespData } from './bindings.ts'
import { useMessage, useNotification } from 'naive-ui'
import LoginDialog from './components/LoginDialog.vue'
import SearchPane from './panes/SearchPane.vue'
import ChapterPane from './panes/ChapterPane.vue'
import DownloadingPane from './panes/DownloadingPane.vue'
import FavoritePane from './panes/FavoritePane.vue'
import { CurrentTabName } from './types.ts'
import DownloadedPane from './panes/DownloadedPane.vue'
import AboutDialog from './components/AboutDialog.vue'
import { PhUser, PhGearSix, PhInfo } from '@phosphor-icons/vue'
import SettingsDialog from './components/SettingsDialog.vue'

const message = useMessage()
const notification = useNotification()

const config = ref<Config>()
const userProfile = ref<UserProfileRespData>()
const loginDialogShowing = ref<boolean>(false)
const aboutDialogShowing = ref<boolean>(false)
const settingsDialogShowing = ref<boolean>(false)
const currentTabName = ref<CurrentTabName>('search')
const pickedComic = ref<Comic>()

watch(
  config,
  async () => {
    if (config.value === undefined) {
      return
    }
    await commands.saveConfig(config.value)
    message.success('保存配置成功')
  },
  { deep: true },
)

watch(
  () => config.value?.token,
  async () => {
    if (config.value === undefined || config.value.token === '') {
      return
    }
    const result = await commands.getUserProfile()
    if (result.status === 'error') {
      notification.error({ title: '获取用户信息失败', description: result.error })
      userProfile.value = undefined
      return
    }
    userProfile.value = result.data
    message.success('获取用户信息成功')
  },
)

onMounted(async () => {
  // 屏蔽浏览器右键菜单
  document.oncontextmenu = (event) => {
    event.preventDefault()
  }
  // 获取配置
  config.value = await commands.getConfig()
})
</script>

<template>
  <div v-if="config !== undefined" class="h-screen flex flex-col">
    <div class="flex gap-col-1 pt-2 px-2">
      <n-input-group>
        <n-input-group-label>Token</n-input-group-label>
        <n-input v-model:value="config.token" placeholder="手动输入或点击右侧的按钮登录" clearable />
        <n-button type="primary" @click="loginDialogShowing = true">
          <template #icon>
            <n-icon>
              <PhUser />
            </n-icon>
          </template>
          登录
        </n-button>
      </n-input-group>
      <div v-if="userProfile !== undefined" class="flex flex-justify-end items-center">
        <n-avatar round :size="32" :src="userProfile.avatar" />
        <span class="whitespace-nowrap">{{ userProfile.nickname }}</span>
      </div>
    </div>
    <div class="flex flex-1 overflow-hidden">
      <n-tabs class="h-full w-1/2" v-model:value="currentTabName" type="line" size="small" animated>
        <n-tab-pane class="h-full overflow-auto p-0!" name="search" tab="漫画搜索" display-directive="show:lazy">
          <search-pane v-model:picked-comic="pickedComic" v-model:current-tab-name="currentTabName" />
        </n-tab-pane>
        <n-tab-pane class="h-full overflow-auto p-0!" name="favorite" tab="漫画收藏" display-directive="show:lazy">
          <favorite-pane
            :user-profile="userProfile"
            v-model:picked-comic="pickedComic"
            v-model:current-tab-name="currentTabName" />
        </n-tab-pane>
        <n-tab-pane class="h-full overflow-auto p-0!" name="downloaded" tab="本地库存" display-directive="show:lazy">
          <downloaded-pane
            v-model:config="config"
            v-model:picked-comic="pickedComic"
            v-model:current-tab-name="currentTabName" />
        </n-tab-pane>
        <n-tab-pane class="h-full overflow-auto p-0!" name="chapter" tab="章节详情" display-directive="show:lazy">
          <chapter-pane v-model:picked-comic="pickedComic" />
        </n-tab-pane>
      </n-tabs>

      <div class="w-1/2 overflow-auto flex flex-col">
        <div
          class="h-8.5 flex gap-col-1 mx-2 items-center border-solid border-0 border-b box-border border-[rgb(239,239,245)]">
          <div class="text-xl font-bold box-border">下载列表</div>
          <div class="flex-1" />
          <n-button size="small" @click="settingsDialogShowing = true">
            <template #icon>
              <n-icon>
                <PhGearSix />
              </n-icon>
            </template>
            配置
          </n-button>
          <n-button size="small" @click="aboutDialogShowing = true">
            <template #icon>
              <n-icon>
                <PhInfo />
              </n-icon>
            </template>
            关于
          </n-button>
        </div>
        <downloading-pane v-model:config="config" />
      </div>
    </div>
    <login-dialog v-model:showing="loginDialogShowing" v-model:config="config" />
    <settings-dialog v-model:showing="settingsDialogShowing" v-model:config="config" />
    <about-dialog v-model:showing="aboutDialogShowing" />
  </div>
</template>

<style scoped>
:global(.n-notification-main__header) {
  @apply break-words;
}

:global(.n-tabs-pane-wrapper) {
  @apply h-full;
}

:deep(.n-tabs-nav) {
  @apply px-2;
}
</style>
