<script setup lang="ts">
import { computed, ref } from 'vue'
import { commands, SearchRespData } from '../bindings.ts'
import { useNotification } from 'naive-ui'
import ComicCard from '../components/ComicCard.vue'
import FloatLabelInput from '../components/FloatLabelInput.vue'
import { PhMagnifyingGlass } from '@phosphor-icons/vue'

const notification = useNotification()

// 搜索输入框的值
const searchInput = ref<string>('')
// 是否正在搜索
const searching = ref<boolean>(false)
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
  searching.value = true

  const result = await commands.search(keyword, page)
  if (result.status === 'error') {
    notification.error({ title: '搜索失败', description: result.error })
    searching.value = false
    return
  }
  searchRespData.value = result.data

  searching.value = false
}
</script>

<template>
  <div class="h-full flex flex-col">
    <n-input-group class="box-border px-2 pt-2">
      <FloatLabelInput
        label="关键词"
        size="small"
        v-model:value="searchInput"
        clearable
        @keydown.enter="search(searchInput.trim(), 1)" />
      <n-button :loading="searching" type="primary" size="small" class="w-15%" @click="search(searchInput.trim(), 1)">
        <template #icon>
          <n-icon size="22">
            <PhMagnifyingGlass />
          </n-icon>
        </template>
      </n-button>
    </n-input-group>
    <div v-if="searchRespData !== undefined" class="flex flex-col gap-row-1 overflow-auto p-2">
      <div class="flex flex-col gap-row-2 overflow-auto pr-2 pb-2">
        <comic-card
          v-for="comicInSearch in searchRespData.list"
          :key="comicInSearch.path_word"
          :comic-info="comicInSearch" />
      </div>
      <n-pagination :page-count="pageCount" :page="currentPage" @update:page="search(searchInput.trim(), $event)" />
    </div>
  </div>
</template>
