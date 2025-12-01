<script setup lang="ts">
import { AuthorRespData, commands } from '../bindings.ts'
import { useStore } from '../store.ts'
import IconButton from './IconButton.vue'
import { PhFolderOpen } from '@phosphor-icons/vue'

const store = useStore()

const props = defineProps<{
  comicTitle: string
  comicPathWord: string
  comicCover: string
  comicAuthor: AuthorRespData[]
  comicDownloaded: boolean
  comicDownloadDir: string
}>()

// 获取漫画信息，将漫画信息存入pickedComic，并切换到章节页
async function pickComic() {
  const getComicResult = await commands.getComic(props.comicPathWord)
  if (getComicResult.status === 'error') {
    console.error(getComicResult.error)
    return
  }
  const comic = getComicResult.data

  store.pickedComic = comic
  store.currentTabName = 'chapter'
  // 如果获取到的漫画已下载的章节，则保存元数据(用于更新元数据)
  if (comic.isDownloaded) {
    const saveMetadataResult = await commands.saveMetadata(comic)
    if (saveMetadataResult.status === 'error') {
      console.error(saveMetadataResult.error)
    }
  }
}

async function showComicDownloadDirInFileManager() {
  const result = await commands.showPathInFileManager(props.comicDownloadDir)
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
        :src="comicCover"
        alt=""
        @click="pickComic" />
      <div class="flex flex-col h-full">
        <span
          class="font-bold text-lg line-clamp-3 cursor-pointer transition-colors duration-200 hover:text-blue-5"
          @click="pickComic">
          {{ comicTitle }}
        </span>
        <span v-html="`作者：${comicAuthor.map((a) => a.name)}`" class="text-red" />
        <IconButton
          class="mt-auto mr-auto"
          v-if="comicDownloaded"
          title="打开下载目录"
          @click="showComicDownloadDirInFileManager">
          <PhFolderOpen :size="24" />
        </IconButton>
      </div>
    </div>
  </n-card>
</template>
