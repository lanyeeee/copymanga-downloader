<script setup lang="ts">
import {SelectionArea, SelectionEvent, SelectionOptions} from "@viselect/vue";
import {computed, nextTick, ref, watch} from "vue";
import {ChapterInfo, Comic, commands} from "../bindings.ts";

const selectedComic = defineModel<Comic | undefined>("selectedComic", {required: true});

const dropdownX = ref<number>(0);
const dropdownY = ref<number>(0);
const showDropdown = ref<boolean>(false);
const dropdownOptions = [
  {label: "勾选", key: "check"},
  {label: "取消勾选", key: "uncheck"},
  {label: "全选", key: "check all"},
  {label: "取消全选", key: "uncheck all"},
];
const checkedIds = ref<string[]>([]);
const selectedIds = ref<Set<string>>(new Set());
const selectionAreaRef = ref<InstanceType<typeof SelectionArea>>();
const currentGroupPath = ref<string>("default");


const currentGroup = computed<ChapterInfo[] | undefined>(() => selectedComic.value?.comic.groups[currentGroupPath.value]);
const sortedGroups = computed<[string, ChapterInfo[]][] | undefined>(() => {
  if (selectedComic.value === undefined) {
    return undefined;
  }

  return Object.entries(selectedComic.value.comic.groups)
      .sort((a, b) => b[1].length - a[1].length);
});
const groupArray = computed<ChapterInfo[] | undefined>(() => {
  const groups = selectedComic.value?.comic.groups;
  if (groups === undefined) {
    return undefined;
  }

  return Object.values(groups).flatMap(infos => infos);
});

watch(selectedComic, () => {
  checkedIds.value.length = 0;
  selectedIds.value.clear();
  selectionAreaRef.value?.selection?.clearSelection();
  currentGroupPath.value = "default";
});

function extractIds(elements: Element[]): string[] {
  return elements.map(element => element.getAttribute("data-key"))
      .filter(Boolean)
      .filter(id => id !== null)
      .filter(id => {
        const chapterInfo = currentGroup.value?.find(chapter => chapter.chapterUuid === id);

        if (chapterInfo === undefined) {
          return false;
        }

        return chapterInfo.isDownloaded === false;
      });
}

function onDragStart({event, selection}: SelectionEvent) {
  if (event?.ctrlKey === false && event?.metaKey === false) {
    selection.clearSelection();
    selectedIds.value.clear();
  }
}

function onDragMove({store: {changed: {added, removed}}}: SelectionEvent) {
  extractIds(added).forEach(id => selectedIds.value.add(id));
  extractIds(removed).forEach(id => selectedIds.value.delete(id));
}

function onDropdownSelect(key: "check" | "uncheck" | "check all" | "uncheck all") {
  showDropdown.value = false;
  if (key === "check") {
    // 只有未勾选的才会被勾选
    [...selectedIds.value]
        .filter(id => checkedIds.value.includes(id) === false)
        .forEach(id => checkedIds.value.push(id));
  } else if (key === "uncheck") {
    checkedIds.value = checkedIds.value.filter(id => selectedIds.value.has(id) === false);
  } else if (key === "check all") {
    // 只对currentGroup中的元素进行勾选
    currentGroup.value
        ?.filter(c => c.isDownloaded === false && checkedIds.value.includes(c.chapterUuid) === false)
        .forEach(c => checkedIds.value.push(c.chapterUuid));
  } else if (key === "uncheck all") {
    // 只对currentGroup中的元素进行取消勾选
    const currentGroupIds = currentGroup.value?.map(c => c.chapterUuid);
    if (currentGroupIds === undefined) {
      return;
    }
    // 删除checkedIds中在currentGroupIds中的元素
    checkedIds.value = checkedIds.value.filter(id => currentGroupIds.includes(id) === false);
  }
}

async function onContextMenu(e: MouseEvent) {
  showDropdown.value = false;
  await nextTick();
  showDropdown.value = true;
  dropdownX.value = e.clientX;
  dropdownY.value = e.clientY;
}

async function downloadChapters() {
  const chapterToDownload = currentGroup.value?.filter(c => c.isDownloaded === false && checkedIds.value.includes(c.chapterUuid));
  if (chapterToDownload === undefined) {
    return;
  }
  await commands.downloadChapters(chapterToDownload);

  for (const downloadedChapter of chapterToDownload) {
    const chapter = currentGroup.value?.find(c => c.chapterUuid === downloadedChapter.chapterUuid);
    if (chapter !== undefined) {
      chapter.isDownloaded = true;
      checkedIds.value = checkedIds.value.filter(id => id !== downloadedChapter.chapterUuid);
    }
  }
}

async function refreshChapters() {
  if (selectedComic.value === undefined) {
    return;
  }

  const result = await commands.getComic(selectedComic.value.comic.path_word);
  if (result.status === "error") {
    console.error(result.error);
    return;
  }

  selectedComic.value = result.data;
}

</script>

<template>
  <div class="h-full flex flex-col">
    <div class="flex flex-justify-around">
      <span>总章数：{{ groupArray?.length }}</span>
      <n-divider vertical></n-divider>
      <span>已下载：{{ groupArray?.filter(c => c.isDownloaded).length }}</span>
      <n-divider vertical></n-divider>
      <span>已勾选：{{ checkedIds.length }}</span>
    </div>
    <div class="flex justify-between">
      左键拖动进行框选，右键打开菜单
      <n-button size="tiny" :disabled="selectedComic===undefined" @click="refreshChapters" class="w-1/6">刷新</n-button>
      <n-button size="tiny" :disabled="selectedComic===undefined" type="primary" @click="downloadChapters"
                class="w-1/4">
        下载勾选章节
      </n-button>
    </div>
    <n-empty v-if="selectedComic === undefined" description="请先进行漫画搜索">
    </n-empty>
    <n-tabs v-else class="flex-1 overflow-auto" v-model:value="currentGroupPath" type="line" size="small">
      <n-tab-pane
          v-for="[groupPath, _] in sortedGroups"
          :key="groupPath"
          :name="groupPath"
          :tab="selectedComic.groups[groupPath].name"
          class="overflow-auto p-0!">
        <SelectionArea ref="selectionAreaRef"
                       class="selection-container h-full"
                       :options="{selectables: '.selectable', features: {deselectOnBlur: true}} as SelectionOptions"
                       @contextmenu="onContextMenu"
                       @move="onDragMove"
                       @start="onDragStart">
          <n-checkbox-group v-model:value="checkedIds" class="grid grid-cols-3 gap-1.5 w-full mb-3">
            <n-checkbox v-for="{chapterUuid, chapterTitle, isDownloaded} in selectedComic.comic.groups[groupPath]"
                        :key="chapterUuid"
                        :data-key="chapterUuid"
                        class="selectable hover:bg-gray-200!"
                        :value="chapterUuid"
                        :label="chapterTitle"
                        :disabled="isDownloaded"
                        :class="{ selected: selectedIds.has(chapterUuid), downloaded: isDownloaded }"/>
          </n-checkbox-group>
        </SelectionArea>
      </n-tab-pane>
    </n-tabs>

    <n-dropdown
        placement="bottom-start"
        trigger="manual"
        :x="dropdownX"
        :y="dropdownY"
        :options="dropdownOptions"
        :show="showDropdown"
        :on-clickoutside="()=>showDropdown=false"
        @select="onDropdownSelect"
    />
  </div>
</template>

<style scoped>
.selection-container {
  @apply user-select-none overflow-auto;
}

.selection-container .selected {
  @apply bg-[rgb(204,232,255)];
}

.selection-container .downloaded {
  @apply bg-[rgba(24,160,88,0.16)];
}

:deep(.n-checkbox__label) {
  @apply overflow-hidden whitespace-nowrap text-ellipsis;
}

:global(.selection-area) {
  @apply bg-[rgba(46,115,252,0.5)];
}
</style>