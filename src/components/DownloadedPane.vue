<script setup lang="ts">
import {Comic, commands} from "../bindings.ts";
import {CurrentTabName} from "../types.ts";
import {ref, watch} from "vue";
import {useNotification} from "naive-ui";
import ComicCard from "./ComicCard.vue";

const notification = useNotification();

const pickedComic = defineModel<Comic | undefined>("pickedComic", {required: true});
const currentTabName = defineModel<CurrentTabName>("currentTabName", {required: true});

const downloadedComics = ref<Comic[]>([]);

watch(() => currentTabName.value, async () => {
  if (currentTabName.value !== "downloaded") {
    return;
  }

  const result = await commands.getDownloadedComics();
  if (result.status === "error") {
    notification.error({title: "获取本地库存失败", description: result.error});
    return;
  }
  downloadedComics.value = result.data;
}, {immediate: true});

async function onClickItem(comicPathWord: string) {
  pickedComic.value = downloadedComics.value.find(({comic}) => comic.path_word === comicPathWord);
  currentTabName.value = "chapter";
}

</script>

<template>
  <div>
    <div class="flex flex-col gap-row-2 overflow-auto">
      <comic-card v-for="{comic} in downloadedComics"
                  :key="comic.uuid"
                  :comic-info="comic"
                  :on-click-item="onClickItem"
                  v-model:picked-comic="pickedComic"
                  v-model:current-tab-name="currentTabName"/>
    </div>
  </div>
</template>