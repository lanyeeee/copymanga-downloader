<script setup lang="ts">
import ComicCard from '../components/ComicCard.vue'
import { computed, ref, watch } from 'vue'
import { commands, GetFavoriteOrdering } from '../bindings.ts'
import { useStore } from '../store.ts'
import { SelectProps } from 'naive-ui'

const store = useStore()

const orderingLabels: Record<GetFavoriteOrdering, string> = {
  Added: '按加到书架时间降序排序',
  Updated: '按作品更新时间降序排序',
  Read: '按上次阅读时间排序',
}
const orderingOptions: SelectProps['options'] = Object.entries(orderingLabels).map(([ordering, label]) => ({
  label: label,
  value: ordering as GetFavoriteOrdering,
}))
const orderingSelected = ref<GetFavoriteOrdering>('Added')

// 当前页码
const currentPage = ref<number>(1)
// 总页数
const pageCount = computed(() => {
  const LIMIT = 18
  if (store.getFavoriteResult === undefined) {
    return 0
  }
  return Math.ceil(store.getFavoriteResult.total / LIMIT)
})
// 如果用户信息变化，重新获取收藏
watch(
  () => store.userProfile,
  async () => {
    if (store.userProfile === undefined) {
      store.getFavoriteResult = undefined
      return
    }
    await getFavorite(1, orderingSelected.value)
  },
  { immediate: true },
)

async function getFavorite(page: number, ordering: GetFavoriteOrdering) {
  orderingSelected.value = ordering
  currentPage.value = page
  const result = await commands.getFavorite(page, ordering)
  if (result.status === 'error') {
    console.error(result.error)
    return
  }
  store.getFavoriteResult = result.data
}
</script>

<template>
  <div class="h-full flex flex-col">
    <div v-if="store.getFavoriteResult !== undefined" class="flex box-border px-2 pt-2">
      <n-input-group>
        <n-input-group-label size="small">排序方式</n-input-group-label>
        <n-select
          class="w-50"
          v-model:value="orderingSelected"
          :options="orderingOptions"
          :show-checkmark="false"
          size="small"
          @update-value="getFavorite(1, $event)" />
      </n-input-group>
    </div>
    <div v-if="store.getFavoriteResult !== undefined" class="flex flex-col gap-row-1 overflow-auto p-2">
      <div class="flex flex-col gap-row-2 overflow-auto pr-2 pb-2">
        <comic-card
          v-for="favoriteItem in store.getFavoriteResult.list"
          :key="favoriteItem.uuid"
          :comic-title="favoriteItem.comic.name"
          :comic-path-word="favoriteItem.comic.pathWord"
          :comic-cover="favoriteItem.comic.cover"
          :comic-author="favoriteItem.comic.author"
          :comic-downloaded="favoriteItem.comic.isDownloaded"
          :comic-download-dir="favoriteItem.comic.comicDownloadDir" />
      </div>
      <n-pagination :page-count="pageCount" :page="currentPage" @update:page="getFavorite($event, orderingSelected)" />
    </div>
  </div>
</template>
