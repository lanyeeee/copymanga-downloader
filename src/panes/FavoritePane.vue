<script setup lang="ts">
import ComicCard from '../components/ComicCard.vue'
import { computed, ref, watch } from 'vue'
import { Comic, commands, GetFavoriteRespData, UserProfileRespData } from '../bindings.ts'
import { useNotification } from 'naive-ui'
import { CurrentTabName } from '../types.ts'

const notification = useNotification()

const props = defineProps<{
  userProfile: UserProfileRespData | undefined
}>()

const pickedComic = defineModel<Comic | undefined>('pickedComic', { required: true })
const currentTabName = defineModel<CurrentTabName>('currentTabName', { required: true })

const getFavoriteRespData = ref<GetFavoriteRespData>()
const pageSelected = ref<number>(1)

const favoritePageCount = computed(() => {
  if (getFavoriteRespData.value === undefined) {
    return 0
  }
  // FIXME: 有潜在的页码错误问题，例如当total为36时，应该返回2，但实际返回3，应该改用向上取整
  return Math.floor(getFavoriteRespData.value.total / 18) + 1
})

watch(
  () => props.userProfile,
  async () => {
    if (props.userProfile === undefined) {
      getFavoriteRespData.value = undefined
      return
    }
    await getFavourite(1)
  },
  { immediate: true },
)

async function getFavourite(page: number) {
  pageSelected.value = page
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
      <div class="flex flex-col gap-row-2 overflow-auto">
        <comic-card
          v-for="favoriteItemRespData in getFavoriteRespData.list"
          :key="favoriteItemRespData.uuid"
          :comic-info="favoriteItemRespData.comic"
          v-model:picked-comic="pickedComic"
          v-model:current-tab-name="currentTabName" />
      </div>
      <n-pagination :page-count="favoritePageCount" :page="pageSelected" @update:page="getFavourite($event)" />
    </div>
  </div>
</template>
