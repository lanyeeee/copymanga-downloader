<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { commands, events } from '../bindings.ts'
import { open } from '@tauri-apps/plugin-dialog'
import { PhFolderOpen } from '@phosphor-icons/vue'
import { useStore } from '../store.ts'
import { ProgressData } from '../types.ts'

const store = useStore()

const downloadSpeed = ref<string>('')

// 章节下载进度
const progresses = ref<Map<string, ProgressData>>(new Map())
// 按总图片数排序的下载进度
const sortedProgresses = computed(() => {
  const progressesArray = Array.from(progresses.value.entries())
  progressesArray.sort((a, b) => {
    return b[1].totalImgCount - a[1].totalImgCount
  })
  return progressesArray
})

onMounted(async () => {
  // 监听下载速度事件
  await events.downloadSpeedEvent.listen(async ({ payload: { speed } }) => {
    downloadSpeed.value = speed
  })
  // 监听下载控制风控事件
  await events.downloadControlRiskEvent.listen(async ({ payload: { chapterUuid, retryAfter } }) => {
    const progressData = progresses.value.get(chapterUuid)
    if (progressData === undefined) {
      return
    }

    progressData.retryAfter = retryAfter
    progressData.indicator = `风控中，将在${retryAfter}秒后自动重试`
  })
  // 监听下载事件
  await events.downloadTaskEvent.listen(({ payload: { event, data } }) => {
    if (event == 'Create') {
      const { chapterInfo, downloadedImgCount, totalImgCount } = data

      progresses.value.set(chapterInfo.chapterUuid, {
        ...data,
        percentage: 0,
        indicator: `排队中 ${downloadedImgCount}/${totalImgCount}`,
        retryAfter: 0,
      })
    } else if (event == 'Update') {
      const { chapterUuid, state, downloadedImgCount, totalImgCount } = data

      const progressData = progresses.value.get(chapterUuid)
      if (progressData === undefined) {
        return
      }

      progressData.state = state
      progressData.downloadedImgCount = downloadedImgCount
      progressData.totalImgCount = totalImgCount
      progressData.percentage = (downloadedImgCount / totalImgCount) * 100

      if (state === 'Completed') {
        progressData.chapterInfo.isDownloaded = true
      }

      let indicator = ''
      if (state === 'Pending') {
        indicator = `排队中`
      } else if (state === 'Downloading') {
        indicator = `下载中`
      } else if (state === 'Paused') {
        indicator = `已暂停`
      } else if (state === 'Cancelled') {
        indicator = `已取消`
      } else if (state === 'Completed') {
        indicator = `下载完成`
      } else if (state === 'Failed') {
        indicator = `下载失败`
      }
      if (totalImgCount !== 0) {
        indicator += ` ${downloadedImgCount}/${totalImgCount}`
      }

      progressData.indicator = indicator
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
    <div
      class="grid grid-cols-[1fr_1fr_2fr] px-2"
      v-for="[
        chapterUuid,
        { comic, chapterInfo, percentage, totalImgCount, retryAfter, indicator },
      ] in sortedProgresses"
      :key="chapterUuid">
      <span class="mb-1! text-ellipsis whitespace-nowrap overflow-hidden">{{ comic.comic.name }}</span>
      <span class="mb-1! text-ellipsis whitespace-nowrap overflow-hidden">{{ chapterInfo.chapterTitle }}</span>
      <div v-if="retryAfter !== 0">{{ indicator }}</div>
      <span v-else-if="totalImgCount === 0">{{ indicator }}</span>
      <n-progress v-else :percentage="percentage">{{ indicator }}</n-progress>
    </div>
  </div>
</template>

<style scoped>
:deep(.n-progress-content) {
  @apply h-full;
}
</style>
