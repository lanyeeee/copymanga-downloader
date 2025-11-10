<script setup lang="tsx">
import { onMounted, ref, watch } from 'vue'
import { commands } from './bindings.ts'
import { useMessage, useNotification } from 'naive-ui'
import LoginDialog from './components/LoginDialog.vue'
import SearchPane from './panes/SearchPane.vue'
import ChapterPane from './panes/ChapterPane.vue'
import DownloadingPane from './panes/DownloadingPane/DownloadingPane.vue'
import FavoritePane from './panes/FavoritePane.vue'
import DownloadedPane from './panes/DownloadedPane.vue'
import AboutDialog from './components/AboutDialog.vue'
import { PhUser, PhGearSix, PhInfo, PhClockCounterClockwise } from '@phosphor-icons/vue'
import SettingsDialog from './components/SettingsDialog.vue'
import { useStore } from './store.ts'
import LogDialog from './components/LogDialog.vue'

const store = useStore()

const message = useMessage()
const notification = useNotification()

const loginDialogShowing = ref<boolean>(false)
const logDialogShowing = ref<boolean>(false)
const settingsDialogShowing = ref<boolean>(false)
const aboutDialogShowing = ref<boolean>(false)

watch(
  () => store.config,
  async () => {
    if (store.config === undefined) {
      return
    }
    await commands.saveConfig(store.config)
    message.success('保存配置成功')
  },
  { deep: true },
)

watch(
  () => store.config?.token,
  async () => {
    if (store.config === undefined || store.config.token === '') {
      return
    }
    const result = await commands.getUserProfile()
    if (result.status === 'error') {
      console.error(result.error)
      store.userProfile = undefined
      return
    }
    store.userProfile = result.data
    message.success('获取用户信息成功')
  },
)

onMounted(async () => {
  // 屏蔽浏览器右键菜单
  document.oncontextmenu = (event) => {
    event.preventDefault()
  }
  // 获取配置
  store.config = await commands.getConfig()
  // 检查日志目录大小
  const result = await commands.getLogsDirSize()
  if (result.status === 'error') {
    console.error(result.error)
    return
  }
  if (result.data > 50 * 1024 * 1024) {
    notification.warning({
      title: '日志目录大小超过50MB，请及时清理日志文件',
      description: () => (
        <>
          <div>
            点击右上角的 <span class="bg-gray-2 px-1">日志</span> 按钮
          </div>
          <div>
            里边有 <span class="bg-gray-2 px-1">打开日志目录</span> 按钮
          </div>
          <div>
            你也可以在里边取消勾选 <span class="bg-gray-2 px-1">输出文件日志</span>
          </div>
          <div>这样将不再产生文件日志</div>
        </>
      ),
    })
  }
})
</script>

<template>
  <div v-if="store.config !== undefined" class="h-screen flex flex-col">
    <div class="flex gap-col-1 pt-2 px-2">
      <n-input-group>
        <n-input-group-label>Token</n-input-group-label>
        <n-input v-model:value="store.config.token" placeholder="手动输入或点击右侧的按钮登录" clearable />
        <n-button type="primary" @click="loginDialogShowing = true">
          <template #icon>
            <n-icon>
              <PhUser />
            </n-icon>
          </template>
          登录
        </n-button>
      </n-input-group>
      <div v-if="store.userProfile !== undefined" class="flex flex-justify-end items-center">
        <n-avatar round :size="32" :src="store.userProfile.avatar" />
        <span class="whitespace-nowrap">{{ store.userProfile.nickname }}</span>
      </div>
    </div>
    <div class="flex flex-1 overflow-hidden">
      <n-tabs class="h-full w-1/2" v-model:value="store.currentTabName" type="line" size="small" animated>
        <n-tab-pane class="h-full overflow-auto p-0!" name="search" tab="漫画搜索" display-directive="show:lazy">
          <search-pane />
        </n-tab-pane>
        <n-tab-pane class="h-full overflow-auto p-0!" name="favorite" tab="漫画收藏" display-directive="show:lazy">
          <favorite-pane />
        </n-tab-pane>
        <n-tab-pane class="h-full overflow-auto p-0!" name="downloaded" tab="本地库存" display-directive="show:lazy">
          <downloaded-pane />
        </n-tab-pane>
        <n-tab-pane class="h-full overflow-auto p-0!" name="chapter" tab="章节详情" display-directive="show:lazy">
          <chapter-pane />
        </n-tab-pane>
      </n-tabs>

      <div class="w-1/2 overflow-auto flex flex-col">
        <div
          class="h-8.5 flex gap-col-1 mx-2 items-center border-solid border-0 border-b box-border border-[rgb(239,239,245)]">
          <div class="text-xl font-bold box-border">下载列表</div>
          <div class="flex-1" />
          <n-button size="small" @click="logDialogShowing = true">
            <template #icon>
              <n-icon>
                <PhClockCounterClockwise />
              </n-icon>
            </template>
            日志
          </n-button>
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
        <downloading-pane />
      </div>
    </div>
    <login-dialog v-model:showing="loginDialogShowing" />
    <log-dialog v-model:showing="logDialogShowing" />
    <settings-dialog v-model:showing="settingsDialogShowing" />
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
