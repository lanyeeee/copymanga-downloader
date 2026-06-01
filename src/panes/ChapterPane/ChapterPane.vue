<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { ChapterInfo, commands } from '../../bindings.ts'
import { useStore } from '../../store.ts'
import { PhFolderOpen } from '@phosphor-icons/vue'
import IconButton from '../../components/IconButton.vue'
import ChapterDownloadPanel from './components/ChapterDownloadPanel.vue'
import ChapterExportPanel from './components/ChapterExportPanel.vue'
import { NEmpty } from 'naive-ui'

export type ChapterPaneMode = 'download' | 'export'

const store = useStore()

const chapterPaneMode = ref<ChapterPaneMode>('download')
const sortedGroups = computed<[string, ChapterInfo[]][]>(() => {
  if (store.pickedComic === undefined) {
    return []
  }

  return Object.entries(store.pickedComic.comic.groups).sort((a, b) => b[1].length - a[1].length)
})
const firstGroupPath = computed(() => sortedGroups.value[0]?.[0] ?? '')
const currentGroupPath = ref<string>(firstGroupPath.value)

watch(
  () => store.pickedComic,
  () => {
    currentGroupPath.value = firstGroupPath.value
    chapterPaneMode.value = 'download'
  },
)

// 重新加载选中的漫画
async function reloadPickedComic() {
  if (store.pickedComic === undefined) {
    return
  }

  const getComicResult = await commands.getComic(store.pickedComic.comic.path_word)
  if (getComicResult.status === 'error') {
    console.error(getComicResult.error)
    return
  }
  const comic = getComicResult.data

  store.pickedComic = getComicResult.data
  if (comic.isDownloaded) {
    const saveMetadataResult = await commands.saveMetadata(comic)
    if (saveMetadataResult.status === 'error') {
      console.error(saveMetadataResult.error)
    }
  }
}

async function showComicDownloadDirInFileManager() {
  if (store.pickedComic === undefined) {
    return
  }

  const comicDownloadDir = store.pickedComic.comicDownloadDir
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
  <div class="h-full flex flex-col box-border">
    <n-empty v-if="store.pickedComic === undefined" description="请先选择漫画(漫画搜索、漫画收藏、本地库存)" />
    <template v-else>
      <ChapterDownloadPanel
        v-if="chapterPaneMode === 'download'"
        v-model:chapterPaneMode="chapterPaneMode"
        v-model:currentGroupPath="currentGroupPath"
        :sortedGroups="sortedGroups"
        :reload="reloadPickedComic" />
      <ChapterExportPanel
        v-else
        v-model:chapterPaneMode="chapterPaneMode"
        v-model:currentGroupPath="currentGroupPath"
        :sortedGroups="sortedGroups"
        :reload="reloadPickedComic" />
    </template>

    <div v-if="store.pickedComic !== undefined" class="flex p-2 pt-0">
      <img class="w-24 mr-4 object-cover" :src="store.pickedComic.comic.cover" alt="" />
      <div class="flex flex-col h-full">
        <span class="font-bold text-xl line-clamp-3">
          {{ store.pickedComic.comic.name }}
        </span>
        <span v-html="`作者：${store.pickedComic.comic.author.map((a) => a.name)}`" class="text-red" />
        <IconButton
          v-if="store.pickedComic.isDownloaded"
          class="mt-auto mr-auto"
          title="打开下载目录"
          @click="showComicDownloadDirInFileManager">
          <PhFolderOpen :size="24" />
        </IconButton>
      </div>
    </div>
  </div>
</template>
