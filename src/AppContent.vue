<script setup lang="ts">
import { onMounted, ref, watch } from 'vue'
import { Comic, commands, Config, UserProfileRespData } from './bindings.ts'
import { useMessage, useNotification } from 'naive-ui'
import LoginDialog from './components/LoginDialog.vue'
import SearchPane from './components/SearchPane.vue'
import ChapterPane from './components/ChapterPane.vue'
import DownloadingList from './components/DownloadingList.vue'
import { appDataDir } from '@tauri-apps/api/path'
import { path } from '@tauri-apps/api'
import FavoritePane from './components/FavoritePane.vue'
import { CurrentTabName } from './types.ts'
import DownloadedPane from './components/DownloadedPane.vue'

const message = useMessage()
const notification = useNotification()

const config = ref<Config>()
const userProfile = ref<UserProfileRespData>()
const loginDialogShowing = ref<boolean>(false)
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

async function showConfigInFileManager() {
  const configName = 'config.json'
  const configPath = await path.join(await appDataDir(), configName)
  const result = await commands.showPathInFileManager(configPath)
  if (result.status === 'error') {
    notification.error({ title: '打开配置文件失败', description: result.error })
  }
}

async function test() {
  if (pickedComic.value === undefined) {
    return
  }
  await commands.saveMetadata(pickedComic.value)
  const result = await commands.getDownloadedComics()
  if (result.status === 'error') {
    notification.error({ title: '获取下载的漫画失败', description: result.error })
    return
  }
  console.log(result.data)
}
</script>

<template>
  <div v-if="config !== undefined" class="h-screen flex flex-col">
    <div class="flex">
      <n-input v-model:value="config.token" placeholder="" clearable>
        <template #prefix>Token：</template>
      </n-input>
      <n-button type="primary" @click="loginDialogShowing = true">账号登录</n-button>
      <n-button @click="showConfigInFileManager">打开配置目录</n-button>
      <n-button @click="test">测试用</n-button>
      <div v-if="userProfile !== undefined" class="flex flex-justify-end">
        <n-avatar round :size="32" :src="userProfile.avatar" />
        <span class="whitespace-nowrap">{{ userProfile.nickname }}</span>
      </div>
    </div>
    <div class="flex flex-1 overflow-hidden">
      <n-tabs class="h-full basis-1/2" v-model:value="currentTabName" type="line" size="small">
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
          <downloaded-pane v-model:picked-comic="pickedComic" v-model:current-tab-name="currentTabName" />
        </n-tab-pane>
        <n-tab-pane class="h-full overflow-auto p-0!" name="chapter" tab="章节详情" display-directive="show:lazy">
          <chapter-pane v-model:picked-comic="pickedComic" />
        </n-tab-pane>
      </n-tabs>
      <downloading-list class="basis-1/2 overflow-auto" v-model:config="config"></downloading-list>
    </div>
    <n-modal v-model:show="loginDialogShowing">
      <login-dialog v-model:showing="loginDialogShowing" v-model:config="config" />
    </n-modal>
  </div>
</template>
