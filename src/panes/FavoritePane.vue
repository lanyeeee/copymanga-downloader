<script setup lang="ts">
import ComicCard from '../components/ComicCard.vue'
import { computed, ref, watch } from 'vue'
import { commands } from '../bindings.ts'
import { useStore } from '../store.ts'

const store = useStore()

// 当前页码
const currentPage = ref<number>(1)
// 总页数
const pageCount = computed(() => {
  if (store.getFavoriteResult === undefined) {
    return 0
  }
  // FIXME: 有潜在的页码错误问题，例如当total为36时，应该返回2，但实际返回3，应该改用向上取整
  return Math.floor(store.getFavoriteResult.total / 18) + 1
})
// 如果用户信息变化，重新获取收藏
watch(
  () => store.userProfile,
  async () => {
    if (store.userProfile === undefined) {
      store.getFavoriteResult = undefined
      return
    }
    await getFavourite(1)
  },
  { immediate: true },
)

async function getFavourite(page: number) {
  currentPage.value = page
  const result = await commands.getFavorite(page)
  if (result.status === 'error') {
    console.error(result.error)
    return
  }
  store.getFavoriteResult = result.data
}
</script>

<template>
  <div class="h-full flex flex-col">
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
      <n-pagination :page-count="pageCount" :page="currentPage" @update:page="getFavourite($event)" />
    </div>
  </div>
</template>
