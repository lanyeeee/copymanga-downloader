<script setup lang="ts">
import { Comic, ComicDetail, commands } from '../bindings.ts'
import { CurrentTabName } from '../types.ts'
import { computed } from 'vue'
import { useNotification } from 'naive-ui'

interface GroupInfo {
  name: string
  downloaded: number
  total: number
}

const notification = useNotification()

const props = defineProps<{
  comic: Comic
}>()

const pickedComic = defineModel<Comic | undefined>('pickedComic', { required: true })
const currentTabName = defineModel<CurrentTabName>('currentTabName', { required: true })

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
  pickedComic.value = props.comic
  currentTabName.value = 'chapter'
}

// 导出cbz
async function exportCbz() {
  const result = await commands.exportCbz(props.comic)
  if (result.status === 'error') {
    notification.error({ title: '导出cbz失败', description: result.error })
    return
  }
}

async function exportPdf() {
  const result = await commands.exportPdf(props.comic)
  if (result.status === 'error') {
    notification.error({ title: '导出pdf失败', description: result.error })
    return
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
        <div class="flex ml-auto mt-auto gap-col-2">
          <n-button size="tiny" @click="exportCbz">导出cbz</n-button>
          <n-button size="tiny" @click="exportPdf">导出pdf</n-button>
        </div>
      </div>
    </div>
  </n-card>
</template>
