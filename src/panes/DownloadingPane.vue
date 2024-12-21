<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { commands, Config, events } from '../bindings.ts'
import { NProgress, useNotification } from 'naive-ui'
import { open } from '@tauri-apps/plugin-dialog'

type ProgressData = {
  comicTitle: string
  chapterTitle: string
  current: number
  total: number
  percentage: number
  indicator: string
  retryAfter: number
}

const notification = useNotification()

const config = defineModel<Config>('config', { required: true })

const progresses = ref<Map<string, ProgressData>>(new Map())
const overallProgress = ref<ProgressData>({
  comicTitle: '总进度',
  chapterTitle: '总进度',
  current: 0,
  total: 0,
  percentage: 0,
  indicator: '',
  retryAfter: 0,
})

const sortedProgresses = computed(() => {
  const progressesArray = Array.from(progresses.value.entries())
  progressesArray.sort((a, b) => {
    return b[1].total - a[1].total
  })
  return progressesArray
})

onMounted(async () => {
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

async function showDownloadDirInFileManager() {
  if (config.value === undefined) {
    return
  }
  const result = await commands.showPathInFileManager(config.value.downloadDir)
  if (result.status === 'error') {
    notification.error({ title: '打开下载目录失败', description: result.error })
  }
}

async function selectDownloadDir() {
  const selectedDirPath = await open({ directory: true })
  if (selectedDirPath === null) {
    return
  }
  config.value.downloadDir = selectedDirPath
}
</script>

<template>
  <div class="flex flex-col gap-row-1">
    <n-h3 class="m-be-0">下载列表</n-h3>
    <div class="flex gap-col-1">
      <n-input
        v-model:value="config.downloadDir"
        size="tiny"
        readonly
        placeholder="请选择漫画目录"
        @click="selectDownloadDir">
        <template #prefix>下载目录：</template>
      </n-input>
      <n-button size="tiny" @click="showDownloadDirInFileManager">打开下载目录</n-button>
    </div>
    <div class="grid grid-cols-[1fr_4fr_2fr]">
      <span class="text-ellipsis whitespace-nowrap overflow-hidden">{{ overallProgress.chapterTitle }}</span>
      <n-progress :percentage="overallProgress.percentage" indicator-placement="inside" :height="21">
        {{ overallProgress.indicator }}
      </n-progress>
      <span>{{ overallProgress.current }}/{{ overallProgress.total }}</span>
    </div>
    <div
      class="grid grid-cols-[1fr_1fr_2fr]"
      v-for="[chapterUuid, { comicTitle, chapterTitle, percentage, current, total, retryAfter }] in sortedProgresses"
      :key="chapterUuid">
      <span class="mb-1! text-ellipsis whitespace-nowrap overflow-hidden">{{ comicTitle }}</span>
      <span class="mb-1! text-ellipsis whitespace-nowrap overflow-hidden">{{ chapterTitle }}</span>
      <div v-if="retryAfter !== 0">风控中，将在{{ retryAfter }}秒后自动重试</div>
      <span v-else-if="total === 0">等待中</span>
      <n-progress v-else class="" :percentage="percentage">{{ current }}/{{ total }}</n-progress>
    </div>
  </div>
</template>
