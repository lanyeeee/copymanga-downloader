<script setup lang="ts">
import ComicCard from "./ComicCard.vue";
import {computed, ref, watch} from "vue";
import {Comic, commands, GetFavoriteRespData, UserProfileRespData} from "../bindings.ts";
import {useNotification} from "naive-ui";

const notification = useNotification();

const props = defineProps<{
  userProfile: UserProfileRespData | undefined
}>();

const selectedComic = defineModel<Comic | undefined>("selectedComic", {required: true});
const currentTabName = defineModel<"search" | "favorite" | "chapter">("currentTabName", {required: true});

const getFavoriteRespData = ref<GetFavoriteRespData>();
const pageSelected = ref<number>(1);

const favoritePageCount = computed(() => {
  if (getFavoriteRespData.value === undefined) {
    return 0;
  }

  return Math.floor(getFavoriteRespData.value.total / 18) + 1;
});

watch(() => props.userProfile, async () => {
  if (props.userProfile === undefined) {
    getFavoriteRespData.value = undefined;
    return;
  }
  await getFavourite(1);
}, {immediate: true});

async function getFavourite(page: number) {
  pageSelected.value = page;
  const result = await commands.getFavorite(page);
  if (result.status === "error") {
    notification.error({title: "获取收藏失败", description: result.error});
    return;
  }
  getFavoriteRespData.value = result.data;
}

</script>

<template>
  <div class="h-full flex flex-col">
    <div v-if="getFavoriteRespData!==undefined" class="flex flex-col gap-row-1 overflow-auto p-2">
      <div class="flex flex-col gap-row-2 overflow-auto">
        <comic-card v-for="favoriteItemRespData in getFavoriteRespData.list"
                    :key="favoriteItemRespData.uuid"
                    :comic-info="favoriteItemRespData.comic"
                    v-model:selected-comic="selectedComic"
                    v-model:current-tab-name="currentTabName"/>
      </div>
      <n-pagination :page-count="favoritePageCount"
                    :page="pageSelected"
                    @update:page="getFavourite($event)"/>
    </div>
  </div>
</template>