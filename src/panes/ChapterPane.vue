<script setup lang="ts">
import { SelectionArea, SelectionEvent, SelectionOptions } from '@viselect/vue'
import { computed, nextTick, ref, watch, watchEffect } from 'vue'
import { ChapterInfo, commands, DownloadTaskState } from '../bindings.ts'
import { useMessage, DropdownOption } from 'naive-ui'
import { useStore } from '../store.ts'
import { PhFolderOpen, PhFilePdf, PhFileArchive } from '@phosphor-icons/vue'
import IconButton from '../components/IconButton.vue'

type State = DownloadTaskState | 'Idle'
type ChapterInfoWithState = ChapterInfo & { state: State }
type Mode = 'download' | 'export'

const store = useStore()

const message = useMessage()

// 当前模式
const currentMode = ref<Mode>('download')
// 当前tab的分组路径
const currentGroupPath = ref<string>('default')
// 当前tab的分组
const currentGroup = computed<ChapterInfoWithState[] | undefined>(() =>
  store.pickedComic?.comic.groups[currentGroupPath.value].map((chapter) => {
    const progressData = store.progresses.get(chapter.chapterUuid)
    return { ...chapter, state: progressData?.state ?? 'Idle' }
  }),
)
// 按章节数排序的分组
const sortedGroups = computed<[string, ChapterInfoWithState[]][] | undefined>(() => {
  if (store.pickedComic === undefined) {
    return undefined
  }

  return Object.entries(store.pickedComic.comic.groups)
    .map(([groupPath, chapters]): [string, ChapterInfoWithState[]] => [
      groupPath,
      chapters.map((chapter) => {
        const progressData = store.progresses.get(chapter.chapterUuid)
        return { ...chapter, state: progressData?.state ?? 'Idle' }
      }),
    ])
    .sort((a, b) => b[1].length - a[1].length)
})

const { dropdownX, dropdownY, dropdownShowing, dropdownOptions, showDropdown } = useDropdown()
const { selectionAreaRef, checkedIds, selectedIds, unselectAll, updateSelectedIds } = useSelectionArea()

// 判断章节是否可选中
function isChapterSelectable(chapter: ChapterInfoWithState): boolean {
  if (isDownloading(chapter.state)) {
    return false
  }
  if (currentMode.value === 'download') {
    return chapter.isDownloaded !== true
  } else {
    // 导出模式：只有已下载的章节可选
    return chapter.isDownloaded === true
  }
}

// 获取章节的CSS类
function getChapterClass(chapter: ChapterInfoWithState): Record<string, boolean> {
  const classes: Record<string, boolean> = {
    selected: selectedIds.value.has(chapter.chapterUuid),
    downloading: isDownloading(chapter.state),
  }

  if (currentMode.value === 'download') {
    classes.downloaded = chapter.isDownloaded === true
  } else {
    // 导出模式：根据导出状态显示不同颜色
    const hasPdf = chapter.isPdfExported === true
    const hasCbz = chapter.isCbzExported === true
    classes.exportedPdf = hasPdf && !hasCbz
    classes.exportedCbz = hasCbz && !hasPdf
    classes.exportedBoth = hasPdf && hasCbz
  }

  return classes
}

function useDropdown() {
  // dropdown的x坐标
  const dropdownX = ref<number>(0)
  // dropdown的y坐标
  const dropdownY = ref<number>(0)
  // 是否显示dropdown
  const dropdownShowing = ref<boolean>(false)
  // dropdown选项 - 根据模式动态计算
  const dropdownOptions = computed<DropdownOption[]>(() => {
    const baseOptions: DropdownOption[] = [
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
    ]

    if (currentMode.value === 'download') {
      baseOptions.push(
        {
          label: '全选未下载章节',
          key: 'check all',
          props: {
            onClick: () => {
              currentGroup.value
                ?.filter((c) => isChapterSelectable(c) && !checkedIds.value.includes(c.chapterUuid))
                .forEach((c) => checkedIds.value.push(c.chapterUuid))
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
      )
    } else {
      baseOptions.push(
        {
          label: '全选已下载章节',
          key: 'check all',
          props: {
            onClick: () => {
              currentGroup.value
                ?.filter((c) => isChapterSelectable(c) && !checkedIds.value.includes(c.chapterUuid))
                .forEach((c) => checkedIds.value.push(c.chapterUuid))
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
      )
    }

    return baseOptions
  })

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
  // 如果漫画变了，清空勾选和选中状态，重置模式
  watch(
    () => store.pickedComic,
    () => {
      checkedIds.value.length = 0
      selectedIds.value.clear()
      selectionAreaRef.value?.selection?.clearSelection()
      currentGroupPath.value = 'default'
      currentMode.value = 'download'
    },
  )

  // 根据模式过滤可勾选的章节
  watchEffect(() => {
    if (store.pickedComic === undefined || sortedGroups.value === undefined) {
      return
    }
    // 只保留可选中状态的章节
    const selectableChapterUuids = sortedGroups.value
      .flatMap(([, chapters]) => chapters)
      .filter((c) => isChapterSelectable(c))
      .map((c) => c.chapterUuid)
    checkedIds.value = checkedIds.value.filter((uuid) => selectableChapterUuids.includes(uuid))
  })

  // 提取章节id
  function extractIds(elements: Element[]): string[] {
    return elements
      .map((element) => element.getAttribute('data-key'))
      .filter(Boolean)
      .filter((id) => {
        const chapterInfo = currentGroup.value?.find((chapter) => chapter.chapterUuid === id)
        return chapterInfo && isChapterSelectable(chapterInfo)
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

// 切换模式时清空勾选和框选状态
watch(currentMode, () => {
  checkedIds.value.length = 0
  selectedIds.value.clear()
  selectionAreaRef.value?.selection?.clearSelection()
})

// 下载勾选的章节
async function downloadChapters() {
  if (store.pickedComic === undefined) {
    message.error('请先选择漫画')
    return
  }
  // 下载勾选的章节
  const chapterUuidsToDownload = currentGroup.value
    ?.filter((c) => isChapterSelectable(c) && checkedIds.value.includes(c.chapterUuid))
    .map((c) => c.chapterUuid)
  if (chapterUuidsToDownload === undefined || chapterUuidsToDownload.length === 0) {
    message.warning('请勾选要下载的章节')
    return
  }
  for (const downloadedChapterUuid of chapterUuidsToDownload) {
    await commands.createDownloadTask(store.pickedComic, downloadedChapterUuid)
  }
  message.success(`已添加 ${chapterUuidsToDownload.length} 个章节到下载队列`)
}

// 导出勾选的章节为PDF
async function exportSelectedAsPdf() {
  if (store.pickedComic === undefined) {
    message.error('请先选择漫画')
    return
  }
  const chapterUuids = currentGroup.value
    ?.filter((c) => isChapterSelectable(c) && checkedIds.value.includes(c.chapterUuid))
    .map((c) => c.chapterUuid)
  if (chapterUuids === undefined || chapterUuids.length === 0) {
    message.warning('请勾选要导出的章节')
    return
  }
  const result = await commands.exportPdfChapters(store.pickedComic, chapterUuids)
  if (result.status === 'error') {
    message.error(result.error.err_message)
    return
  }
  message.success(`已开始导出 ${chapterUuids.length} 个章节为PDF`)
  // 切换到进度tab
  store.progressesPaneTabName = 'uncompleted'
}

// 导出勾选的章节为CBZ
async function exportSelectedAsCbz() {
  if (store.pickedComic === undefined) {
    message.error('请先选择漫画')
    return
  }
  const chapterUuids = currentGroup.value
    ?.filter((c) => isChapterSelectable(c) && checkedIds.value.includes(c.chapterUuid))
    .map((c) => c.chapterUuid)
  if (chapterUuids === undefined || chapterUuids.length === 0) {
    message.warning('请勾选要导出的章节')
    return
  }
  const result = await commands.exportCbzChapters(store.pickedComic, chapterUuids)
  if (result.status === 'error') {
    message.error(result.error.err_message)
    return
  }
  message.success(`已开始导出 ${chapterUuids.length} 个章节为CBZ`)
  // 切换到进度tab
  store.progressesPaneTabName = 'uncompleted'
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
  const comic = getComicResult.data

  store.pickedComic = getComicResult.data
  // 如果获取到的漫画已下载的章节，则保存元数据(用于更新元数据)
  if (comic.isDownloaded) {
    const saveMetadataResult = await commands.saveMetadata(comic)
    if (saveMetadataResult.status === 'error') {
      console.error(saveMetadataResult.error)
    }
  }
}

async function showComicDownloadDirInFileManager() {
  if (store.pickedComic === undefined) {
    return
  }

  const comicDownloadDir = store.pickedComic.comicDownloadDir
  if (comicDownloadDir === undefined || comicDownloadDir === null) {
    console.error('comicDownloadDir的值为undefined或null')
    return
  }

  const result = await commands.showPathInFileManager(comicDownloadDir)
  if (result.status === 'error') {
    console.error(result.error)
  }
}

function isDownloading(state: State) {
  return state === 'Pending' || state === 'Downloading' || state === 'Paused'
}
</script>

<template>
  <div class="h-full flex flex-col box-border">
    <div v-if="store.pickedComic !== undefined" class="flex items-center select-none pt-2 gap-1 px-2">
      <n-radio-group v-model:value="currentMode" size="small">
        <n-radio-button value="download">下载</n-radio-button>
        <n-radio-button value="export">导出</n-radio-button>
      </n-radio-group>
      <n-button class="ml-auto" size="small" @click="reloadPickedComic">刷新</n-button>
      <template v-if="currentMode === 'download'">
        <n-button size="small" type="primary" @click="downloadChapters">下载勾选章节</n-button>
      </template>
      <template v-else>
        <n-button size="small" type="info" @click="exportSelectedAsPdf">
          <template #icon>
            <PhFilePdf :size="16" />
          </template>
          导出PDF
        </n-button>
        <n-button size="small" type="info" @click="exportSelectedAsCbz">
          <template #icon>
            <PhFileArchive :size="16" />
          </template>
          导出CBZ
        </n-button>
      </template>
    </div>
    <n-empty v-if="store.pickedComic === undefined" description="请先选择漫画(漫画搜索、漫画收藏、本地库存)" />
    <n-tabs v-else class="flex-1 overflow-auto" v-model:value="currentGroupPath" type="line" size="small" animated>
      <n-tab-pane
        v-for="[groupPath, chapters] in sortedGroups"
        :key="groupPath"
        :name="groupPath"
        :tab="store.pickedComic.groups[groupPath].name"
        class="overflow-auto p-0! h-full">
        <SelectionArea
          ref="selectionAreaRef"
          class="selection-container flex flex-col flex-1 box-border pt-2 px-2 overflow-auto h-full"
          :options="{ selectables: '.selectable', features: { deselectOnBlur: true } } as SelectionOptions"
          @contextmenu="showDropdown"
          @move="updateSelectedIds"
          @start="unselectAll">
          <n-checkbox-group v-model:value="checkedIds" class="grid grid-cols-3 gap-1.5 w-full mb-3">
            <n-checkbox
              v-for="chapter in chapters"
              :key="chapter.chapterUuid"
              :data-key="chapter.chapterUuid"
              class="selectable hover:bg-gray-200!"
              :value="chapter.chapterUuid"
              :label="chapter.chapterTitle"
              :disabled="!isChapterSelectable(chapter)"
              :class="getChapterClass(chapter)" />
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
        <span v-html="`作者：${store.pickedComic.comic.author.map((a) => a.name)}`" class="text-red" />
        <IconButton
          v-if="store.pickedComic.isDownloaded"
          class="mt-auto mr-auto"
          title="打开下载目录"
          @click="showComicDownloadDirInFileManager">
          <PhFolderOpen :size="24" />
        </IconButton>
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

.selection-container .downloaded {
  @apply bg-[rgba(24,160,88,0.16)];
}

.selection-container .downloading {
  @apply bg-[rgba(114,46,209,0.16)];
}

.selection-container .exportedPdf {
  @apply bg-[rgba(234,88,12,0.16)];
}

.selection-container .exportedCbz {
  @apply bg-[rgba(59,130,246,0.16)];
}

.selection-container .exportedBoth {
  @apply bg-[rgba(6,182,212,0.16)];
}

.selection-container .selected {
  @apply bg-[rgb(204,232,255)] !important;
}

:deep(.n-checkbox__label) {
  @apply overflow-hidden whitespace-nowrap text-ellipsis;
}

:global(.selection-area) {
  @apply bg-[rgba(46,115,252,0.5)];
}
</style>
