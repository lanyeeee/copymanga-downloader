<script setup lang="ts">
import { Comic, commands, events } from '../bindings.ts'
import { computed, onMounted, ref, watch } from 'vue'
import { MessageReactive } from 'naive-ui'
import DownloadedComicCard from '../components/DownloadedComicCard.vue'
import { open } from '@tauri-apps/plugin-dialog'
import { PhFolderOpen } from '@phosphor-icons/vue'
import { useStore } from '../store.ts'
import { useMessage, useNotification } from 'naive-ui'

const store = useStore()

const message = useMessage()
const notification = useNotification()

const PAGE_SIZE = 20
// 已下载的漫画
const downloadedComics = ref<Comic[]>([])
// 当前页码
const currentPage = ref<number>(1)
// 总页数
const pageCount = computed(() => {
  return Math.ceil(downloadedComics.value.length / PAGE_SIZE)
})
// 当前页的漫画
const currentPageComics = computed(() => {
  const start = (currentPage.value - 1) * PAGE_SIZE
  const end = start + PAGE_SIZE
  return downloadedComics.value.slice(start, end)
})

// 监听标签页变化，更新下载的漫画列表
watch(
  () => store.currentTabName,
  async () => {
    if (store.currentTabName !== 'downloaded') {
      return
    }

    downloadedComics.value = await commands.getDownloadedComics()
  },
  { immediate: true },
)

// 用文件管理器打开导出目录
async function showExportDirInFileManager() {
  if (store.config === undefined) {
    return
  }
  const result = await commands.showPathInFileManager(store.config.exportDir)
  if (result.status === 'error') {
    console.error(result.error)
  }
}

// 选择导出目录
async function selectExportDir() {
  if (store.config === undefined) {
    return
  }

  const selectedDirPath = await open({ directory: true })
  if (selectedDirPath === null) {
    return
  }
  store.config.exportDir = selectedDirPath
}

let updateMessage: MessageReactive | undefined

onMounted(async () => {
  await events.updateDownloadedComicsEvent.listen(async ({ payload: updateEvent }) => {
    if (updateEvent.event === 'GettingComics') {
      const { total } = updateEvent.data
      updateMessage = message.loading(`正在获取已下载漫画的最新数据(0/${total})`, { duration: 0 })
    } else if (updateEvent.event === 'GetComicError' && updateMessage !== undefined) {
      const { comicTitle, errMsg } = updateEvent.data
      notification.warning({
        title: `获取漫画 ${comicTitle} 的数据失败`,
        description: errMsg,
        duration: 0,
      })
    } else if (updateEvent.event === 'ComicGot' && updateMessage !== undefined) {
      const { current, total } = updateEvent.data
      updateMessage.content = `正在获取已下载漫画的最新数据(${current}/${total})`
    } else if (updateEvent.event === 'DownloadTaskCreated' && updateMessage !== undefined) {
      updateMessage.type = 'success'
      updateMessage.content = '已为需要更新的章节创建下载任务'
      setTimeout(() => {
        updateMessage?.destroy()
        updateMessage = undefined
      }, 3000)
    }
  })
})

// 更新已下载漫画
async function updateDownloadedComics() {
  const result = await commands.updateDownloadedComics()
  if (result.status === 'error') {
    setTimeout(() => {
      updateMessage?.destroy()
      updateMessage = undefined
    }, 3000)
    console.error(result.error)
  }
}
</script>

<template>
  <div v-if="store.config !== undefined" class="h-full flex flex-col overflow-auto gap-row-1">
    <div class="flex gap-1 box-border px-2 pt-2">
      <n-input-group>
        <n-input-group-label size="small">导出目录</n-input-group-label>
        <n-input v-model:value="store.config.exportDir" size="small" readonly @click="selectExportDir" />
        <n-button class="w-10" size="small" @click="showExportDirInFileManager">
          <template #icon>
            <n-icon size="20">
              <PhFolderOpen />
            </n-icon>
          </template>
        </n-button>
      </n-input-group>
      <n-button size="small" @click="updateDownloadedComics">更新库存</n-button>
    </div>

    <div class="flex flex-col gap-row-1 overflow-auto p-2 pt-0">
      <div class="flex flex-col gap-row-2 overflow-auto pr-2 pb-2">
        <downloaded-comic-card v-for="comic in currentPageComics" :key="comic.comic.uuid" :comic="comic" />
      </div>
      <n-pagination :page-count="pageCount" :page="currentPage" @update:page="currentPage = $event" />
    </div>
  </div>
</template>
