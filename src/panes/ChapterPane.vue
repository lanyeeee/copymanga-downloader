<script setup lang="ts">
import { SelectionArea, SelectionEvent, SelectionOptions } from '@viselect/vue'
import { computed, nextTick, ref, watch } from 'vue'
import { ChapterInfo, commands } from '../bindings.ts'
import { useMessage, DropdownOption } from 'naive-ui'
import { useStore } from '../store.ts'

const store = useStore()

const message = useMessage()

const { currentGroupPath, currentGroup, sortedGroups, downloadChapters } = useChapters()
const { dropdownX, dropdownY, dropdownShowing, dropdownOptions, showDropdown } = useDropdown()
const { selectionAreaRef, checkedIds, selectedIds, unselectAll, updateSelectedIds } = useSelectionArea()

function useChapters() {
  // 当前tab的分组路径
  const currentGroupPath = ref<string>('default')
  // 当前tab的分组
  const currentGroup = computed<ChapterInfo[] | undefined>(
    () => store.pickedComic?.comic.groups[currentGroupPath.value],
  )
  // 按章节数排序的分组
  const sortedGroups = computed<[string, ChapterInfo[]][] | undefined>(() => {
    if (store.pickedComic === undefined) {
      return undefined
    }

    return Object.entries(store.pickedComic.comic.groups).sort((a, b) => b[1].length - a[1].length)
  })

  // 下载勾选的章节
  async function downloadChapters() {
    if (store.pickedComic === undefined) {
      message.error('请先选择漫画')
      return
    }
    // 创建下载任务前，先创建元数据
    const result = await commands.saveMetadata(store.pickedComic)
    if (result.status === 'error') {
      console.error(result.error)
      return
    }
    // 下载勾选的章节
    const chapterUuidsToDownload = currentGroup.value
      ?.filter((c) => c.isDownloaded === false && checkedIds.value.includes(c.chapterUuid))
      .map((c) => c.chapterUuid)
    if (chapterUuidsToDownload === undefined) {
      return
    }
    for (const downloadedChapterUuid of chapterUuidsToDownload) {
      await commands.createDownloadTask(store.pickedComic, downloadedChapterUuid)
      const chapter = currentGroup.value?.find((c) => c.chapterUuid === downloadedChapterUuid)
      if (chapter !== undefined) {
        chapter.isDownloaded = true
      }
    }
  }

  return { currentGroupPath, currentGroup, sortedGroups, downloadChapters }
}

function useDropdown() {
  // dropdown的x坐标
  const dropdownX = ref<number>(0)
  // dropdown的y坐标
  const dropdownY = ref<number>(0)
  // 是否显示dropdown
  const dropdownShowing = ref<boolean>(false)
  // dropdown选项
  const dropdownOptions: DropdownOption[] = [
    {
      label: '勾选',
      key: 'check',
      props: {
        onClick: () => {
          // 只有未勾选的才会被勾选
          ;[...selectedIds.value]
            .filter((id) => !checkedIds.value.includes(id))
            .forEach((id) => checkedIds.value.push(id))
          dropdownShowing.value = false
        },
      },
    },
    {
      label: '取消勾选',
      key: 'uncheck',
      props: {
        onClick: () => {
          checkedIds.value = checkedIds.value.filter((id) => !selectedIds.value.has(id))
          dropdownShowing.value = false
        },
      },
    },
    {
      label: '全选',
      key: 'check all',
      props: {
        onClick: () => {
          currentGroup.value
            // TODO: 改用 === false，不要用 !，因为isDownloaded可能是undefined和null
            ?.filter((c) => !c.isDownloaded && !checkedIds.value.includes(c.chapterUuid))
            .forEach((c) => checkedIds.value.push(c.chapterUuid))
          // TODO: 可以考虑下面这种写法
          // const currentGroupIds = currentGroup.value?.map((c) => c.chapterUuid) ?? []
          // checkedIds.value = [...new Set([...checkedIds.value, ...currentGroupIds])]
          dropdownShowing.value = false
        },
      },
    },
    {
      label: '取消全选',
      key: 'uncheck all',
      props: {
        onClick: () => {
          const currentGroupIds = currentGroup.value?.map((c) => c.chapterUuid) ?? []
          checkedIds.value = checkedIds.value.filter((id) => !currentGroupIds.includes(id))
          dropdownShowing.value = false
        },
      },
    },
  ]

  // 显示dropdown
  async function showDropdown(e: MouseEvent) {
    dropdownShowing.value = false
    await nextTick()
    dropdownShowing.value = true
    dropdownX.value = e.clientX
    dropdownY.value = e.clientY
  }

  return { dropdownX, dropdownY, dropdownShowing, dropdownOptions, showDropdown }
}

function useSelectionArea() {
  // 已勾选的章节id
  const checkedIds = ref<string[]>([])
  // 已选中(被框选选到)的章节id
  const selectedIds = ref<Set<string>>(new Set())
  // SelectionArea组件的ref
  const selectionAreaRef = ref<InstanceType<typeof SelectionArea>>()
  // 如果漫画变了，清空勾选和选中状态
  watch(
    () => store.pickedComic,
    () => {
      checkedIds.value.length = 0
      selectedIds.value.clear()
      selectionAreaRef.value?.selection?.clearSelection()
      currentGroupPath.value = 'default'
    },
  )

  // 提取章节id
  function extractIds(elements: Element[]): string[] {
    return elements
      .map((element) => element.getAttribute('data-key'))
      .filter(Boolean)
      .filter((id) => {
        const chapterInfo = currentGroup.value?.find((chapter) => chapter.chapterUuid === id)
        return chapterInfo && !chapterInfo.isDownloaded // TODO: 改用 === false，不要用 !，因为isDownloaded可能是undefined和null
      }) as string[]
  }

  // 取消所有已选中(被框选选到)的章节
  function unselectAll({ event, selection }: SelectionEvent) {
    if (!event?.ctrlKey && !event?.metaKey) {
      selection.clearSelection()
      selectedIds.value.clear()
    }
  }

  // 更新已选中(被框选选到)的章节id
  function updateSelectedIds({
    store: {
      changed: { added, removed },
    },
  }: SelectionEvent) {
    extractIds(added).forEach((id) => selectedIds.value.add(id))
    extractIds(removed).forEach((id) => selectedIds.value.delete(id))
  }

  return { selectionAreaRef, checkedIds, selectedIds, unselectAll, updateSelectedIds }
}

// 重新加载选中的漫画
async function reloadPickedComic() {
  if (store.pickedComic === undefined) {
    return
  }

  const getComicResult = await commands.getComic(store.pickedComic.comic.path_word)
  if (getComicResult.status === 'error') {
    console.error(getComicResult.error)
    return
  }

  store.pickedComic = getComicResult.data
  // 如果获取到的漫画中有已下载的章节，则保存元数据
  let chapterInfos = Object.values(getComicResult.data.comic.groups).flat()
  if (chapterInfos.some((chapterInfo) => chapterInfo.isDownloaded)) {
    const saveMetadataResult = await commands.saveMetadata(getComicResult.data)
    if (saveMetadataResult.status === 'error') {
      console.error(saveMetadataResult.error)
    }
  }
}
</script>

<template>
  <div class="h-full flex flex-col">
    <div v-if="store.pickedComic !== undefined" class="flex items-center select-none pt-2 gap-1 px-2">
      左键拖动进行框选，右键打开菜单
      <n-button class="ml-auto" size="small" @click="reloadPickedComic">刷新</n-button>
      <n-button size="small" type="primary" @click="downloadChapters">下载勾选章节</n-button>
    </div>
    <n-empty v-if="store.pickedComic === undefined" description="请先选择漫画(漫画搜索、漫画收藏、本地库存)" />
    <n-tabs v-else class="flex-1 overflow-auto" v-model:value="currentGroupPath" type="line" size="small">
      <n-tab-pane
        v-for="[groupPath] in sortedGroups"
        :key="groupPath"
        :name="groupPath"
        :tab="store.pickedComic.groups[groupPath].name"
        class="overflow-auto p-0! h-full">
        <SelectionArea
          ref="selectionAreaRef"
          class="selection-container flex flex-col flex-1 px-2 overflow-auto h-full"
          :options="{ selectables: '.selectable', features: { deselectOnBlur: true } } as SelectionOptions"
          @contextmenu="showDropdown"
          @move="updateSelectedIds"
          @start="unselectAll">
          <n-checkbox-group v-model:value="checkedIds" class="grid grid-cols-3 gap-1.5 w-full mb-3">
            <n-checkbox
              v-for="{ chapterUuid, chapterTitle, isDownloaded } in store.pickedComic.comic.groups[groupPath]"
              :key="chapterUuid"
              :data-key="chapterUuid"
              class="selectable hover:bg-gray-200!"
              :value="chapterUuid"
              :label="chapterTitle"
              :disabled="isDownloaded"
              :class="{ selected: selectedIds.has(chapterUuid), downloaded: isDownloaded }" />
          </n-checkbox-group>
        </SelectionArea>
      </n-tab-pane>
    </n-tabs>
    <div v-if="store.pickedComic !== undefined" class="flex p-2 pt-0">
      <img class="w-24 mr-4 object-cover" :src="store.pickedComic?.comic.cover" alt="" />
      <div class="flex flex-col h-full">
        <span class="font-bold text-xl line-clamp-3">
          {{ store.pickedComic.comic.name }}
        </span>
        <span v-html="`作者：${store.pickedComic.comic.author.map((a) => a.name)}`" class="text-red"></span>
      </div>
    </div>

    <n-dropdown
      placement="bottom-start"
      trigger="manual"
      :x="dropdownX"
      :y="dropdownY"
      :options="dropdownOptions"
      :show="dropdownShowing"
      :on-clickoutside="() => (dropdownShowing = false)" />
  </div>
</template>

<style scoped>
.selection-container {
  @apply select-none overflow-auto;
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
