<script setup lang="ts">
import { Comic, commands, events } from '../bindings.ts'
import { computed, onMounted, ref, watch } from 'vue'
import { MessageReactive, useMessage, useNotification } from 'naive-ui'
import DownloadedComicCard from '../components/DownloadedComicCard.vue'
import { open } from '@tauri-apps/plugin-dialog'
import { PhFolderOpen } from '@phosphor-icons/vue'
import { useStore } from '../store.ts'

interface ProgressData {
  comicTitle: string
  current: number
  total: number
  progressMessage: MessageReactive
}

const store = useStore()

const notification = useNotification()
const message = useMessage()

const { currentPage, pageCount, currentPageComics } = useDownloadedComics()
useProgressTracking()

function useDownloadedComics() {
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

      const result = await commands.getDownloadedComics()
      if (result.status === 'error') {
        console.error(result.error)
        return
      }
      downloadedComics.value = result.data
    },
    { immediate: true },
  )

  return { currentPage, pageCount, currentPageComics }
}

let updateMessage: MessageReactive | undefined

function useProgressTracking() {
  const progresses = ref<Map<string, ProgressData>>(new Map())

  // 处理导出CBZ事件
  async function handleExportCbzEvents() {
    await events.exportCbzEvent.listen(async ({ payload: exportEvent }) => {
      if (exportEvent.event === 'Start') {
        const { uuid, comicTitle, total } = exportEvent.data
        createProgress(uuid, comicTitle, total, '正在导出cbz')
      } else if (exportEvent.event === 'Progress') {
        updateProgress(exportEvent.data)
      } else if (exportEvent.event === 'Error') {
        failProgress(exportEvent.data.uuid, '导出cbz失败')
      } else if (exportEvent.event === 'End') {
        completeProgress(exportEvent.data.uuid, '导出cbz完成')
      }
    })
  }

  // 处理导出PDF事件
  async function handleExportPdfEvents() {
    await events.exportPdfEvent.listen(async ({ payload: exportEvent }) => {
      if (exportEvent.event === 'CreateStart') {
        const { uuid, comicTitle, total } = exportEvent.data
        createProgress(uuid, comicTitle, total, '正在创建pdf')
      } else if (exportEvent.event === 'CreateProgress') {
        updateProgress(exportEvent.data)
      } else if (exportEvent.event === 'CreateError') {
        failProgress(exportEvent.data.uuid, '创建pdf失败')
      } else if (exportEvent.event === 'CreateEnd') {
        completeProgress(exportEvent.data.uuid, '创建pdf完成')
      } else if (exportEvent.event === 'MergeStart') {
        const { uuid, comicTitle, total } = exportEvent.data
        createProgress(uuid, comicTitle, total, '正在合并pdf')
      } else if (exportEvent.event === 'MergeProgress') {
        updateProgress(exportEvent.data)
      } else if (exportEvent.event === 'MergeError') {
        failProgress(exportEvent.data.uuid, '合并pdf失败')
      } else if (exportEvent.event === 'MergeEnd') {
        completeProgress(exportEvent.data.uuid, '合并pdf完成')
      }
    })
  }

  // 处理更新已下载漫画事件
  async function handleUpdateEvents() {
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
  }

  // 创建进度message
  function createProgress(uuid: string, comicTitle: string, total: number, actionMessage: string) {
    progresses.value.set(uuid, {
      comicTitle,
      current: 0,
      total,
      progressMessage: message.loading(
        () => {
          const progressData = progresses.value.get(uuid)
          if (progressData === undefined) return ''
          return `${progressData.comicTitle} ${actionMessage}(${progressData.current}/${progressData.total})`
        },
        { duration: 0 },
      ),
    })
  }

  // 更新进度message
  function updateProgress({ uuid, current }: { uuid: string; current: number }) {
    const progressData = progresses.value.get(uuid)
    if (progressData) {
      progressData.current = current
    }
  }

  // 将进度message标记为完成
  function completeProgress(uuid: string, actionMessage: string) {
    const progressData = progresses.value.get(uuid)
    if (progressData) {
      progressData.progressMessage.type = 'success'
      progressData.progressMessage.content = `${progressData.comicTitle} ${actionMessage}(${progressData.current}/${progressData.total})`
      setTimeout(() => {
        progressData.progressMessage.destroy()
        progresses.value.delete(uuid)
      }, 3000)
    }
  }

  // 将进度message标记为失败
  function failProgress(uuid: string, errorMessage: string) {
    const progressData = progresses.value.get(uuid)
    if (progressData) {
      progressData.progressMessage.type = 'error'
      progressData.progressMessage.content = `${progressData.comicTitle} 导出失败(${progressData.current}/${progressData.total}): ${errorMessage}`
      setTimeout(() => {
        progressData.progressMessage.destroy()
        progresses.value.delete(uuid)
      }, 3000)
    }
  }

  // 监听导出事件
  onMounted(async () => {
    await handleExportCbzEvents()
    await handleExportPdfEvents()
    await handleUpdateEvents()
  })
}

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
