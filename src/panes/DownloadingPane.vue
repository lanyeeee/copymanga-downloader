<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { commands, events } from '../bindings.ts'
import { useNotification } from 'naive-ui'
import { open } from '@tauri-apps/plugin-dialog'
import { PhFolderOpen } from '@phosphor-icons/vue'
import { useStore } from '../store.ts'

type ProgressData = {
  comicTitle: string
  chapterTitle: string
  current: number
  total: number
  percentage: number
  indicator: string
  retryAfter: number
}

const store = useStore()

const notification = useNotification()

// 章节下载进度
const progresses = ref<Map<string, ProgressData>>(new Map())
// 总下载进度
const overallProgress = ref<ProgressData>({
  comicTitle: '总进度',
  chapterTitle: '总进度',
  current: 0,
  total: 0,
  percentage: 0,
  indicator: '',
  retryAfter: 0,
})
// 按总图片数排序的下载进度
const sortedProgresses = computed(() => {
  const progressesArray = Array.from(progresses.value.entries())
  progressesArray.sort((a, b) => {
    return b[1].total - a[1].total
  })
  return progressesArray
})

onMounted(async () => {
  // 监听下载事件
  await events.downloadEvent.listen(({ payload: downloadEvent }) => {
    if (downloadEvent.event == 'ChapterPending') {
      const { chapterUuid, comicTitle, chapterTitle } = downloadEvent.data
      let progressData: ProgressData = {
        comicTitle,
        chapterTitle,
        current: 0,
        total: 0,
        percentage: 0,
        indicator: '',
        retryAfter: 0,
      }
      progresses.value.set(chapterUuid, progressData)
    } else if (downloadEvent.event == 'ChapterControlRisk') {
      const { chapterUuid, retryAfter } = downloadEvent.data
      const progressData = progresses.value.get(chapterUuid) as ProgressData | undefined
      if (progressData === undefined) {
        return
      }
      progressData.retryAfter = retryAfter
    } else if (downloadEvent.event == 'ChapterStart') {
      const { chapterUuid, total } = downloadEvent.data
      const progressData = progresses.value.get(chapterUuid) as ProgressData | undefined
      if (progressData === undefined) {
        return
      }
      progressData.total = total
    } else if (downloadEvent.event == 'ChapterEnd') {
      const { chapterUuid, errMsg } = downloadEvent.data
      const progressData = progresses.value.get(chapterUuid) as ProgressData | undefined
      if (progressData === undefined) {
        return
      }
      if (errMsg !== null) {
        notification.warning({
          title: '下载章节失败',
          content: errMsg,
          meta: `${progressData.comicTitle} - ${progressData.chapterTitle}`,
        })
      }
      progresses.value.delete(chapterUuid)
    } else if (downloadEvent.event == 'ImageSuccess') {
      const { chapterUuid, current } = downloadEvent.data
      const progressData = progresses.value.get(chapterUuid) as ProgressData | undefined
      if (progressData === undefined) {
        return
      }
      progressData.current = current
      progressData.percentage = Math.round((progressData.current / progressData.total) * 100)
    } else if (downloadEvent.event == 'ImageError') {
      const { chapterUuid, url, errMsg } = downloadEvent.data
      const progressData = progresses.value.get(chapterUuid) as ProgressData | undefined
      if (progressData === undefined) {
        return
      }
      notification.warning({
        title: '下载图片失败',
        description: url,
        content: errMsg,
        meta: `${progressData.comicTitle} - ${progressData.chapterTitle}`,
      })
    } else if (downloadEvent.event == 'OverallSpeed') {
      const { speed } = downloadEvent.data
      overallProgress.value.indicator = speed
    } else if (downloadEvent.event == 'OverallUpdate') {
      const { percentage, downloadedImageCount, totalImageCount } = downloadEvent.data
      overallProgress.value.percentage = percentage
      overallProgress.value.current = downloadedImageCount
      overallProgress.value.total = totalImageCount
    }
  })
})

// 用文件管理器打开下载目录
async function showDownloadDirInFileManager() {
  if (store.config === undefined) {
    return
  }

  const result = await commands.showPathInFileManager(store.config.downloadDir)
  if (result.status === 'error') {
    console.error(result.error)
  }
}

// 通过对话框选择下载目录
async function selectDownloadDir() {
  if (store.config === undefined) {
    return
  }

  const selectedDirPath = await open({ directory: true })
  if (selectedDirPath === null) {
    return
  }
  store.config.downloadDir = selectedDirPath
}
</script>

<template>
  <div v-if="store.config !== undefined" class="flex flex-col">
    <n-input-group class="box-border px-2 pt-2">
      <n-input-group-label size="small">下载目录</n-input-group-label>
      <n-input v-model:value="store.config.downloadDir" size="small" readonly @click="selectDownloadDir" />
      <n-button class="w-10" size="small" @click="showDownloadDirInFileManager">
        <template #icon>
          <n-icon size="20">
            <PhFolderOpen />
          </n-icon>
        </template>
      </n-button>
    </n-input-group>
    <div class="grid grid-cols-[1fr_4fr_2fr] px-2">
      <span class="text-ellipsis whitespace-nowrap overflow-hidden">{{ overallProgress.chapterTitle }}</span>
      <n-progress :percentage="overallProgress.percentage" indicator-placement="inside" :height="16">
        {{ overallProgress.indicator }}
      </n-progress>
      <span>{{ overallProgress.current }}/{{ overallProgress.total }}</span>
    </div>
    <div
      class="grid grid-cols-[1fr_1fr_2fr] px-2"
      v-for="[chapterUuid, { comicTitle, chapterTitle, percentage, current, total, retryAfter }] in sortedProgresses"
      :key="chapterUuid">
      <span class="mb-1! text-ellipsis whitespace-nowrap overflow-hidden">{{ comicTitle }}</span>
      <span class="mb-1! text-ellipsis whitespace-nowrap overflow-hidden">{{ chapterTitle }}</span>
      <div v-if="retryAfter !== 0">风控中，将在{{ retryAfter }}秒后自动重试</div>
      <span v-else-if="total === 0">等待中</span>
      <n-progress v-else :percentage="percentage">{{ current }}/{{ total }}</n-progress>
    </div>
  </div>
</template>

<style scoped>
:deep(.n-progress-content) {
  @apply h-full;
}
</style>
