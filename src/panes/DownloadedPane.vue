<script setup lang="ts">
import { Comic, commands, Config, events } from '../bindings.ts'
import { CurrentTabName } from '../types.ts'
import { computed, onMounted, ref, watch } from 'vue'
import { MessageReactive, useMessage, useNotification } from 'naive-ui'
import DownloadedComicCard from '../components/DownloadedComicCard.vue'
import { open } from '@tauri-apps/plugin-dialog'

interface ProgressData {
  comicTitle: string
  current: number
  total: number
  progressMessage: MessageReactive
}

const notification = useNotification()
const message = useMessage()

const config = defineModel<Config>('config', { required: true })
const pickedComic = defineModel<Comic | undefined>('pickedComic', { required: true })
const currentTabName = defineModel<CurrentTabName>('currentTabName', { required: true })

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
    () => currentTabName.value,
    async () => {
      if (currentTabName.value !== 'downloaded') {
        return
      }

      const result = await commands.getDownloadedComics()
      if (result.status === 'error') {
        useNotification().error({ title: '获取本地库存失败', description: result.error })
        return
      }
      downloadedComics.value = result.data
    },
    { immediate: true },
  )

  return { currentPage, pageCount, currentPageComics }
}

function useProgressTracking() {
  // TODO: 没必要用ref包装Map，直接用Map就行
  const progresses = ref<Map<string, ProgressData>>(new Map())

  // 处理导出CBZ事件
  async function handleExportCbzEvents() {
    await events.exportCbzEvent.listen(async ({ payload: exportEvent }) => {
      if (exportEvent.event === 'Start') {
        const { uuid, comicTitle, total } = exportEvent.data
        createProgress(uuid, comicTitle, total, '正在导出cbz')
      } else if (exportEvent.event === 'Progress') {
        updateProgress(exportEvent.data)
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
      } else if (exportEvent.event === 'CreateEnd') {
        completeProgress(exportEvent.data.uuid, '创建pdf完成')
      } else if (exportEvent.event === 'MergeStart') {
        const { uuid, comicTitle, total } = exportEvent.data
        createProgress(uuid, comicTitle, total, '正在合并pdf')
      } else if (exportEvent.event === 'MergeProgress') {
        updateProgress(exportEvent.data)
      } else if (exportEvent.event === 'MergeEnd') {
        completeProgress(exportEvent.data.uuid, '合并pdf完成')
      }
    })
  }

  let updateMessage: MessageReactive | undefined

  // 处理更新已下载漫画事件
  async function handleUpdateEvents() {
    await events.updateDownloadedComicsEvent.listen(async ({ payload: updateEvent }) => {
      if (updateEvent.event === 'GettingComics') {
        const { total } = updateEvent.data
        updateMessage = message.loading(`正在获取已下载漫画的最新数据(0/${total})`, { duration: 0 })
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

  // 监听导出事件
  onMounted(async () => {
    await handleExportCbzEvents()
    await handleExportPdfEvents()
    await handleUpdateEvents()
  })
}

// 用文件管理器打开导出目录
async function showExportDirInFileManager() {
  if (config.value === undefined) {
    return
  }
  const result = await commands.showPathInFileManager(config.value.exportDir)
  if (result.status === 'error') {
    notification.error({ title: '打开导出目录失败', description: result.error })
  }
}

// 选择导出目录
async function selectExportDir() {
  const selectedDirPath = await open({ directory: true })
  if (selectedDirPath === null) {
    return
  }
  config.value.exportDir = selectedDirPath
}

// 更新已下载漫画
async function updateDownloadedComics() {
  const result = await commands.updateDownloadedComics()
  if (result.status === 'error') {
    notification.error({ title: '更新库存漫画失败', description: result.error, duration: 0 })
  }
}
</script>

<template>
  <div class="h-full flex flex-col overflow-auto gap-row-1">
    <div class="flex gap-col-1">
      <n-input
        v-model:value="config.exportDir"
        size="tiny"
        readonly
        placeholder="请选择漫画目录"
        @click="selectExportDir">
        <template #prefix>导出目录</template>
      </n-input>
      <n-button size="tiny" @click="showExportDirInFileManager">打开目录</n-button>
      <n-button size="tiny" @click="updateDownloadedComics">更新库存</n-button>
    </div>
    <div class="flex flex-col gap-row-1 overflow-auto p-2 pt-0">
      <div class="flex flex-col gap-row-2 overflow-auto pr-2 pb-2">
        <downloaded-comic-card
          v-for="comic in currentPageComics"
          :key="comic.comic.uuid"
          :comic="comic"
          v-model:picked-comic="pickedComic"
          v-model:current-tab-name="currentTabName" />
      </div>
      <n-pagination :page-count="pageCount" :page="currentPage" @update:page="currentPage = $event" />
    </div>
  </div>
</template>
