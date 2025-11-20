<script setup lang="ts">
import { Comic, ComicDetail, commands } from '../bindings.ts'
import { computed } from 'vue'
import { useStore } from '../store.ts'

interface GroupInfo {
  name: string
  downloaded: number
  total: number
}

const store = useStore()

const props = defineProps<{
  comic: Comic
}>()

const comicDetail = computed<ComicDetail>(() => props.comic.comic)
const groupInfos = computed<GroupInfo[]>(() => {
  const groups = comicDetail.value.groups

  const infos = Object.values(groups).map((chapterInfos) => {
    const groupInfo: GroupInfo = {
      name: chapterInfos[0].groupName,
      downloaded: chapterInfos.filter((chapterInfo) => chapterInfo.isDownloaded).length,
      total: chapterInfos.length,
    }
    return groupInfo
  })

  infos.sort((a, b) => b.total - a.total)
  return infos
})

// 选中漫画，切换到章节页
async function pickComic() {
  store.pickedComic = props.comic
  store.currentTabName = 'chapter'
}

// 导出cbz
async function exportCbz() {
  store.progressesPaneTabName = 'export'
  const result = await commands.exportCbz(props.comic)
  if (result.status === 'error') {
    console.error(result.error)
    return
  }
}

async function exportPdf() {
  store.progressesPaneTabName = 'export'
  const result = await commands.exportPdf(props.comic)
  if (result.status === 'error') {
    console.error(result.error)
    return
  }
}

async function showComicDownloadDirInFileManager() {
  const comicDownloadDir = props.comic.comicDownloadDir
  if (comicDownloadDir === undefined || comicDownloadDir === null) {
    console.error('comicDownloadDir的值为undefined或null')
    return
  }

  const result = await commands.showPathInFileManager(comicDownloadDir)
  if (result.status === 'error') {
    console.error(result.error)
  }
}
</script>

<template>
  <n-card content-style="padding: 0.25rem;" hoverable>
    <div class="flex h-full">
      <img
        class="w-24 object-cover mr-4 cursor-pointer transition-transform duration-200 hover:scale-106"
        :src="comicDetail.cover"
        alt=""
        @click="pickComic" />
      <div class="flex flex-col h-full w-full">
        <span
          class="font-bold text-xl line-clamp-3 cursor-pointer transition-colors duration-200 hover:text-blue-5"
          @click="pickComic">
          {{ comicDetail.name }}
        </span>
        <span v-html="`作者：${comicDetail.author.map((a) => a.name)}`" class="text-red"></span>
        <span v-for="groupInfo in groupInfos" :key="groupInfo.name">
          {{ groupInfo.name }}({{ groupInfo.downloaded }}/{{ groupInfo.total }})
        </span>
        <div class="flex mt-auto gap-col-2">
          <n-button size="tiny" @click="showComicDownloadDirInFileManager">打开下载目录</n-button>
          <n-button class="ml-auto" size="tiny" @click="exportCbz">导出cbz</n-button>
          <n-button size="tiny" @click="exportPdf">导出pdf</n-button>
        </div>
      </div>
    </div>
  </n-card>
</template>
