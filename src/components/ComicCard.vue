<script setup lang="ts">
import {Comic, commands} from "../bindings.ts";
import {useNotification} from "naive-ui";
import {ComicInfo} from "../types.ts";

defineProps<{
  comicInfo: ComicInfo
}>();

const selectedComic = defineModel<Comic | undefined>("selectedComic", {required: true});
const currentTabName = defineModel<"search" | "favorite" | "chapter">("currentTabName", {required: true});

const notification = useNotification();

async function onClickItem(comic_id: string) {
  const result = await commands.getComic(comic_id);
  if (result.status === "error") {
    notification.error({title: "获取漫画失败", description: result.error});
    return;
  }
  selectedComic.value = result.data;
  currentTabName.value = "chapter";
}

</script>

<template>
  <n-card content-style="padding: 0.25rem;" hoverable>
    <div class="flex">
      <img class="w-24 object-cover mr-4 cursor-pointer transition-transform duration-200 hover:scale-106"
           :src="comicInfo.cover"
           alt=""
           @click="onClickItem(comicInfo.path_word)"/>
      <div class="flex flex-col h-full">
        <span class="font-bold text-xl line-clamp-3 cursor-pointer transition-colors duration-200 hover:text-blue-5"
              @click="onClickItem(comicInfo.path_word)">
          {{ comicInfo.name }}
        </span>
        <span v-html="`作者：${comicInfo.author.map(a => a.name)}`" class="text-red"></span>
      </div>
    </div>
  </n-card>
</template>