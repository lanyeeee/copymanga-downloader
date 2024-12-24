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

const PAGE_SIZE = 20

const notification = useNotification()
const message = useMessage()

const config = defineModel<Config>('config', { required: true })
const pickedComic = defineModel<Comic | undefined>('pickedComic', { required: true })
const currentTabName = defineModel<CurrentTabName>('currentTabName', { required: true })

const downloadedComics = ref<Comic[]>([])
const pageSelected = ref<number>(1)
const progresses = ref<Map<string, ProgressData>>(new Map())

const downloadedPageCount = computed(() => {
  return Math.ceil(downloadedComics.value.length / PAGE_SIZE)
})
// 当前在页面上显示的漫画
const showingDownloadedComics = computed(() => {
  const start = (pageSelected.value - 1) * PAGE_SIZE
  const end = start + PAGE_SIZE
  return downloadedComics.value.slice(start, end)
})

watch(
  () => currentTabName.value,
  async () => {
    if (currentTabName.value !== 'downloaded') {
      return
    }

    const result = await commands.getDownloadedComics()
    if (result.status === 'error') {
      notification.error({ title: '获取本地库存失败', description: result.error })
      return
    }
    downloadedComics.value = result.data
  },
  { immediate: true },
)

onMounted(async () => {
  await events.exportCbzEvent.listen(async ({ payload: exportEvent }) => {
    if (exportEvent.event === 'Start') {
      const { uuid, comicTitle, total } = exportEvent.data
      progresses.value.set(uuid, {
        comicTitle,
        current: 0,
        total,
        progressMessage: message.loading(
          () => {
            const progressData = progresses.value.get(uuid)
            if (progressData === undefined) {
              return ''
            }
            return `${progressData.comicTitle} 正在导出cbz(${progressData.current}/${progressData.total})`
          },
          { duration: 0 },
        ),
      })
    } else if (exportEvent.event === 'Progress') {
      const { uuid, current } = exportEvent.data
      const progressData = progresses.value.get(uuid)
      if (progressData === undefined) {
        return
      }
      progressData.current = current
    } else if (exportEvent.event === 'End') {
      const { uuid } = exportEvent.data
      const progressData = progresses.value.get(uuid)
      if (progressData === undefined) {
        return
      }
      progressData.progressMessage.type = 'success'
      progressData.progressMessage.content = `${progressData.comicTitle} 导出cbz完成(${progressData.current}/${progressData.total})`
      setTimeout(() => {
        progressData.progressMessage.destroy()
        progresses.value.delete(uuid)
      }, 3000)
    }
  })
})

async function showExportDirInFileManager() {
  if (config.value === undefined) {
    return
  }
  const result = await commands.showPathInFileManager(config.value.exportDir)
  if (result.status === 'error') {
    notification.error({ title: '打开导出目录失败', description: result.error })
  }
}

async function selectExportDir() {
  const selectedDirPath = await open({ directory: true })
  if (selectedDirPath === null) {
    return
  }
  config.value.exportDir = selectedDirPath
}
</script>

<template>
  <div class="h-full flex flex-col overflow-auto">
    <div class="flex gap-col-1">
      <n-input
        v-model:value="config.exportDir"
        size="tiny"
        readonly
        placeholder="请选择漫画目录"
        @click="selectExportDir">
        <template #prefix>导出目录：</template>
      </n-input>
      <n-button size="tiny" @click="showExportDirInFileManager">打开导出目录</n-button>
    </div>
    <div class="flex flex-col gap-row-1 overflow-auto p-2">
      <div class="flex flex-col gap-row-2 overflow-auto">
        <downloaded-comic-card
          v-for="comic in showingDownloadedComics"
          :key="comic.comic.uuid"
          :comic="comic"
          v-model:picked-comic="pickedComic"
          v-model:current-tab-name="currentTabName" />
      </div>
      <n-pagination :page-count="downloadedPageCount" :page="pageSelected" @update:page="pageSelected = $event" />
    </div>
  </div>
</template>
