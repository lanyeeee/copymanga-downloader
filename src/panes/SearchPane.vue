<script setup lang="ts">
import { computed, ref } from 'vue'
import { Comic, commands, SearchRespData } from '../bindings.ts'
import { useNotification } from 'naive-ui'
import ComicCard from '../components/ComicCard.vue'
import { CurrentTabName } from '../types.ts'

const notification = useNotification()

const pickedComic = defineModel<Comic | undefined>('pickedComic', { required: true })
const currentTabName = defineModel<CurrentTabName>('currentTabName', { required: true })
// 搜索输入框的值
const searchInput = ref<string>('')
// 当前页码
const currentPage = ref<number>(1)
// 搜索返回的数据
const searchRespData = ref<SearchRespData>()
// 总页数
const pageCount = computed(() => {
  const LIMIT = 20
  if (searchRespData.value === undefined) {
    return 0
  }
  const total = searchRespData.value.total
  return Math.floor(total / LIMIT) + 1
})

async function search(keyword: string, page: number) {
  console.log(keyword, page)
  currentPage.value = page
  const result = await commands.search(keyword, page)
  if (result.status === 'error') {
    notification.error({ title: '搜索失败', description: result.error })
    return
  }
  searchRespData.value = result.data
}
</script>

<template>
  <div class="h-full flex flex-col">
    <div class="flex">
      <n-input
        class="text-align-left"
        size="tiny"
        v-model:value="searchInput"
        placeholder=""
        clearable
        @keydown.enter="search(searchInput.trim(), 1)">
        <template #prefix>关键词:</template>
      </n-input>
      <n-button size="tiny" @click="search(searchInput.trim(), 1)">搜索</n-button>
    </div>
    <div v-if="searchRespData !== undefined" class="flex flex-col gap-row-1 overflow-auto p-2">
      <div class="flex flex-col gap-row-2 overflow-auto pr-2 pb-2">
        <comic-card
          v-for="comicInSearch in searchRespData.list"
          :key="comicInSearch.path_word"
          :comic-info="comicInSearch"
          v-model:picked-comic="pickedComic"
          v-model:current-tab-name="currentTabName" />
      </div>
      <n-pagination :page-count="pageCount" :page="currentPage" @update:page="search(searchInput.trim(), $event)" />
    </div>
  </div>
</template>
