<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { commands, events } from '../../bindings.ts'
import { open } from '@tauri-apps/plugin-dialog'
import { PhFolderOpen } from '@phosphor-icons/vue'
import { useStore } from '../../store.ts'
import UncompletedProgresses from './components/UncompletedProgresses.vue'
import CompletedProgresses from './components/CompletedProgresses.vue'
import { ProgressData } from '../../types.ts'

const store = useStore()

const tabName = ref<'uncompleted' | 'completed'>('uncompleted')
const downloadSpeed = ref<string>('')

onMounted(async () => {
  // 监听下载速度事件
  await events.downloadSpeedEvent.listen(async ({ payload: { speed } }) => {
    downloadSpeed.value = speed
  })
  // 监听下载控制风控事件
  await events.downloadControlRiskEvent.listen(async ({ payload: { chapterUuid, retryAfter } }) => {
    const progressData = store.progresses.get(chapterUuid)
    if (progressData === undefined) {
      return
    }

    progressData.retryAfter = retryAfter
    progressData.indicator = `风控中，将在${retryAfter}秒后自动重试`
  })
  // 监听下载事件
  await events.downloadTaskEvent.listen(async ({ payload: { event, data } }) => {
    if (event == 'Create') {
      const { chapterInfo, downloadedImgCount, totalImgCount } = data

      store.progresses.set(chapterInfo.chapterUuid, {
        ...data,
        percentage: 0,
        indicator: `排队中 ${downloadedImgCount}/${totalImgCount}`,
        retryAfter: 0,
      })
    } else if (event == 'Update') {
      const { chapterUuid, state, downloadedImgCount, totalImgCount } = data

      const progressData = store.progresses.get(chapterUuid)
      if (progressData === undefined) {
        return
      }

      progressData.state = state
      progressData.downloadedImgCount = downloadedImgCount
      progressData.totalImgCount = totalImgCount
      progressData.percentage = (downloadedImgCount / totalImgCount) * 100

      if (state === 'Completed') {
        progressData.chapterInfo.isDownloaded = true
        await syncPickedComic()
        await syncComicInSearch(progressData)
        await syncComicInFavorite(progressData)
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

async function syncPickedComic() {
  if (store.pickedComic === undefined) {
    return
  }
  const result = await commands.getSyncedComic(store.pickedComic)
  if (result.status === 'error') {
    console.error(result.error)
    return
  }
  Object.assign(store.pickedComic, { ...result.data })
}

async function syncComicInSearch(progressData: ProgressData) {
  if (store.searchResult === undefined) {
    return
  }
  const comic = store.searchResult.list.find((comic) => comic.pathWord === progressData.comic.comic.path_word)
  if (comic === undefined) {
    return
  }
  const result = await commands.getSyncedComicInSearch(comic)
  if (result.status === 'error') {
    console.error(result.error)
    return
  }
  Object.assign(comic, { ...result.data })
}

async function syncComicInFavorite(progressData: ProgressData) {
  if (store.getFavoriteResult === undefined) {
    return
  }
  const comic = store.getFavoriteResult.list
    .map((favoriteItem) => favoriteItem.comic)
    .find((comic) => comic.uuid === progressData.comic.comic.uuid)
  if (comic === undefined) {
    return
  }
  const result = await commands.getSyncedComicInFavorite(comic)
  if (result.status === 'error') {
    console.error(result.error)
    return
  }
  Object.assign(comic, { ...result.data })
}

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
  <div v-if="store.config !== undefined" class="flex flex-col flex-1 overflow-auto">
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
    <n-tabs class="h-full overflow-auto" v-model:value="tabName" type="line" size="small">
      <n-tab-pane class="h-full p-0! overflow-auto" name="uncompleted" tab="未完成">
        <uncompleted-progresses />
      </n-tab-pane>
      <n-tab-pane class="h-full p-0! overflow-auto" name="completed" tab="已完成">
        <completed-progresses />
      </n-tab-pane>

      <template #suffix>
        <span class="whitespace-nowrap text-ellipsis overflow-hidden">{{ downloadSpeed }}</span>
      </template>
    </n-tabs>
  </div>
</template>
