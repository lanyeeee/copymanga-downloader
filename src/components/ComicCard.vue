<script setup lang="ts">
import { Comic, commands } from '../bindings.ts'
import { useNotification } from 'naive-ui'
import { ComicInfo, CurrentTabName } from '../types.ts'

const props = defineProps<{
  comicInfo: ComicInfo
}>()

const pickedComic = defineModel<Comic | undefined>('pickedComic', { required: true })
const currentTabName = defineModel<CurrentTabName>('currentTabName', { required: true })

const notification = useNotification()

// 获取漫画信息，将漫画信息存入pickedComic，并切换到章节页
async function pickComic() {
  const getComicResult = await commands.getComic(props.comicInfo.path_word)
  if (getComicResult.status === 'error') {
    notification.error({ title: '获取漫画失败', description: getComicResult.error })
    return
  }

  pickedComic.value = getComicResult.data
  currentTabName.value = 'chapter'
  // 如果获取到的漫画中有已下载的章节，则保存元数据
  let chapterInfos = Object.values(getComicResult.data.comic.groups).flat()
  if (chapterInfos.some((chapterInfo) => chapterInfo.isDownloaded)) {
    const saveMetadataResult = await commands.saveMetadata(getComicResult.data)
    if (saveMetadataResult.status === 'error') {
      notification.error({ title: '保存元数据失败', description: saveMetadataResult.error })
    }
  }
}
</script>

<template>
  <n-card content-style="padding: 0.25rem;" hoverable>
    <div class="flex">
      <img
        class="w-24 object-cover mr-4 cursor-pointer transition-transform duration-200 hover:scale-106"
        :src="comicInfo.cover"
        alt=""
        @click="pickComic" />
      <div class="flex flex-col h-full">
        <span
          class="font-bold text-xl line-clamp-3 cursor-pointer transition-colors duration-200 hover:text-blue-5"
          @click="pickComic">
          {{ comicInfo.name }}
        </span>
        <span v-html="`作者：${comicInfo.author.map((a) => a.name)}`" class="text-red"></span>
      </div>
    </div>
  </n-card>
</template>
