<script setup lang="ts">
import ComicCard from '../components/ComicCard.vue'
import { computed, ref, watch } from 'vue'
import { commands, GetFavoriteRespData } from '../bindings.ts'
import { useNotification } from 'naive-ui'
import { useStore } from '../store.ts'

const store = useStore()

const notification = useNotification()

// 获取收藏返回的数据
const getFavoriteRespData = ref<GetFavoriteRespData>()
// 当前页码
const currentPage = ref<number>(1)
// 总页数
const pageCount = computed(() => {
  if (getFavoriteRespData.value === undefined) {
    return 0
  }
  // FIXME: 有潜在的页码错误问题，例如当total为36时，应该返回2，但实际返回3，应该改用向上取整
  return Math.floor(getFavoriteRespData.value.total / 18) + 1
})
// 如果用户信息变化，重新获取收藏
watch(
  () => store.userProfile,
  async () => {
    if (store.userProfile === undefined) {
      getFavoriteRespData.value = undefined
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
    notification.error({ title: '获取收藏失败', description: result.error })
    return
  }
  getFavoriteRespData.value = result.data
}
</script>

<template>
  <div class="h-full flex flex-col">
    <div v-if="getFavoriteRespData !== undefined" class="flex flex-col gap-row-1 overflow-auto p-2">
      <div class="flex flex-col gap-row-2 overflow-auto pr-2 pb-2">
        <comic-card
          v-for="favoriteItemRespData in getFavoriteRespData.list"
          :key="favoriteItemRespData.uuid"
          :comic-info="favoriteItemRespData.comic" />
      </div>
      <n-pagination :page-count="pageCount" :page="currentPage" @update:page="getFavourite($event)" />
    </div>
  </div>
</template>
