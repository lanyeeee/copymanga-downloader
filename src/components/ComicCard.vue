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

async function pickComic() {
  const result = await commands.getComic(props.comicInfo.path_word)
  if (result.status === 'error') {
    notification.error({ title: '获取漫画失败', description: result.error })
    return
  }
  pickedComic.value = result.data
  currentTabName.value = 'chapter'
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
