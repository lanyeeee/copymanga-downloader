<script setup lang="ts">
import { SelectionArea, SelectionEvent, SelectionOptions } from '@viselect/vue'
import { computed, nextTick, ref, watch } from 'vue'
import { ChapterInfo, Comic, commands } from '../bindings.ts'
import { useMessage, useNotification } from 'naive-ui'

const notification = useNotification()
const message = useMessage()

const pickedComic = defineModel<Comic | undefined>('pickedComic', { required: true })

const { currentGroupPath, currentGroup, sortedGroups, chapterInfos, downloadChapters } = useChapters()
const { dropdownX, dropdownY, dropdownShowing, dropdownOptions, showDropdown, onDropdownSelect } = useDropdown()
const { selectionAreaRef, checkedIds, selectedIds, unselectAll, updateSelectedIds } = useSelectionArea()

function useChapters() {
  // 当前tab的分组路径
  const currentGroupPath = ref<string>('default')
  // 当前tab的分组
  const currentGroup = computed<ChapterInfo[] | undefined>(
    () => pickedComic.value?.comic.groups[currentGroupPath.value],
  )
  // 按章节数排序的分组
  const sortedGroups = computed<[string, ChapterInfo[]][] | undefined>(() => {
    if (pickedComic.value === undefined) {
      return undefined
    }

    return Object.entries(pickedComic.value.comic.groups).sort((a, b) => b[1].length - a[1].length)
  })
  // 所有章节
  const chapterInfos = computed<ChapterInfo[] | undefined>(() => {
    const groups = pickedComic.value?.comic.groups
    if (groups === undefined) {
      return undefined
    }

    return Object.values(groups).flatMap((infos) => infos)
  })

  // 下载勾选的章节
  async function downloadChapters() {
    if (pickedComic.value === undefined) {
      message.error('请先选择漫画')
      return
    }
    // 创建下载任务前，先创建元数据
    const result = await commands.saveMetadata(pickedComic.value)
    if (result.status === 'error') {
      notification.error({ title: '保存元数据失败', description: result.error })
      return
    }
    // 下载勾选的章节
    // FIXME: 应该下载所有勾选的章节，而不是只下载当前分组勾选的章节，把currentGroup改为chapterInfos
    const chapterToDownload = currentGroup.value?.filter(
      (c) => c.isDownloaded === false && checkedIds.value.includes(c.chapterUuid),
    )
    if (chapterToDownload === undefined) {
      return
    }
    await commands.downloadChapters(chapterToDownload)
    // 更新勾选状态
    for (const downloadedChapter of chapterToDownload) {
      const chapter = currentGroup.value?.find((c) => c.chapterUuid === downloadedChapter.chapterUuid)
      if (chapter !== undefined) {
        chapter.isDownloaded = true
        checkedIds.value = checkedIds.value.filter((id) => id !== downloadedChapter.chapterUuid)
      }
    }
  }

  return { currentGroupPath, currentGroup, sortedGroups, chapterInfos, downloadChapters }
}

function useDropdown() {
  // dropdown的x坐标
  const dropdownX = ref<number>(0)
  // dropdown的y坐标
  const dropdownY = ref<number>(0)
  // 是否显示dropdown
  const dropdownShowing = ref<boolean>(false)
  // dropdown选项
  const dropdownOptions = [
    { label: '勾选', key: 'check' },
    { label: '取消勾选', key: 'uncheck' },
    { label: '全选', key: 'check all' },
    { label: '取消全选', key: 'uncheck all' },
  ]

  // 显示dropdown
  async function showDropdown(e: MouseEvent) {
    dropdownShowing.value = false
    await nextTick()
    dropdownShowing.value = true
    dropdownX.value = e.clientX
    dropdownY.value = e.clientY
  }

  // dropdown选项点击事件
  function onDropdownSelect(key: 'check' | 'uncheck' | 'check all' | 'uncheck all') {
    dropdownShowing.value = false
    if (key === 'check') {
      ;[...selectedIds.value].filter((id) => !checkedIds.value.includes(id)).forEach((id) => checkedIds.value.push(id))
    } else if (key === 'uncheck') {
      checkedIds.value = checkedIds.value.filter((id) => !selectedIds.value.has(id))
    } else if (key === 'check all') {
      currentGroup.value
        ?.filter((c) => !c.isDownloaded && !checkedIds.value.includes(c.chapterUuid))
        .forEach((c) => checkedIds.value.push(c.chapterUuid))
    } else if (key === 'uncheck all') {
      const currentGroupIds = currentGroup.value?.map((c) => c.chapterUuid) ?? []
      checkedIds.value = checkedIds.value.filter((id) => !currentGroupIds.includes(id))
    }
  }

  return { dropdownX, dropdownY, dropdownShowing, dropdownOptions, showDropdown, onDropdownSelect }
}

function useSelectionArea() {
  // 已勾选的章节id
  const checkedIds = ref<string[]>([])
  // 已选中(被框选选到)的章节id
  const selectedIds = ref<Set<string>>(new Set())
  // SelectionArea组件的ref
  const selectionAreaRef = ref<InstanceType<typeof SelectionArea>>()
  // 如果漫画变了，清空勾选和选中状态
  watch(pickedComic, () => {
    checkedIds.value.length = 0
    selectedIds.value.clear()
    selectionAreaRef.value?.selection?.clearSelection()
    currentGroupPath.value = 'default'
  })

  // 提取章节id
  function extractIds(elements: Element[]): string[] {
    return elements
      .map((element) => element.getAttribute('data-key'))
      .filter(Boolean)
      .filter((id) => {
        const chapterInfo = currentGroup.value?.find((chapter) => chapter.chapterUuid === id)
        return chapterInfo && !chapterInfo.isDownloaded
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
  if (pickedComic.value === undefined) {
    return
  }

  const result = await commands.getComic(pickedComic.value.comic.path_word)
  if (result.status === 'error') {
    console.error(result.error)
    return
  }

  pickedComic.value = result.data
}
</script>

<template>
  <div class="h-full flex flex-col">
    <div class="flex flex-justify-around">
      <span>总章数：{{ chapterInfos?.length }}</span>
      <n-divider vertical></n-divider>
      <span>已下载：{{ chapterInfos?.filter((c) => c.isDownloaded).length }}</span>
      <n-divider vertical></n-divider>
      <span>已勾选：{{ checkedIds.length }}</span>
    </div>
    <div class="flex justify-between">
      左键拖动进行框选，右键打开菜单
      <n-button size="tiny" :disabled="pickedComic === undefined" @click="reloadPickedComic" class="w-1/6">
        刷新
      </n-button>
      <n-button
        size="tiny"
        :disabled="pickedComic === undefined"
        type="primary"
        @click="downloadChapters"
        class="w-1/4">
        下载勾选章节
      </n-button>
    </div>
    <n-empty v-if="pickedComic === undefined" description="请先选择漫画(漫画搜索、漫画收藏、本地库存)"></n-empty>
    <n-tabs v-else class="flex-1 overflow-auto" v-model:value="currentGroupPath" type="line" size="small">
      <n-tab-pane
        v-for="[groupPath, _] in sortedGroups"
        :key="groupPath"
        :name="groupPath"
        :tab="pickedComic.groups[groupPath].name"
        class="overflow-auto p-0!">
        <SelectionArea
          ref="selectionAreaRef"
          class="selection-container h-full"
          :options="{ selectables: '.selectable', features: { deselectOnBlur: true } } as SelectionOptions"
          @contextmenu="showDropdown"
          @move="updateSelectedIds"
          @start="unselectAll">
          <n-checkbox-group v-model:value="checkedIds" class="grid grid-cols-3 gap-1.5 w-full mb-3">
            <n-checkbox
              v-for="{ chapterUuid, chapterTitle, isDownloaded } in pickedComic.comic.groups[groupPath]"
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

    <n-dropdown
      placement="bottom-start"
      trigger="manual"
      :x="dropdownX"
      :y="dropdownY"
      :options="dropdownOptions"
      :show="dropdownShowing"
      :on-clickoutside="() => (dropdownShowing = false)"
      @select="onDropdownSelect" />
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
