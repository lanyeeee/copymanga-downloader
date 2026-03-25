<script setup lang="tsx">
import { PartialSelectionOptions, SelectionArea, SelectionEvent } from '@viselect/vue'
import { ChapterInfo, commands, DownloadTaskState } from '../../../bindings.ts'
import {
  DropdownOption,
  NButton,
  NCheckbox,
  NDropdown,
  NIcon,
  NPopover,
  NRadioButton,
  NRadioGroup,
  NTabPane,
  NTabs,
} from 'naive-ui'
import { useStore } from '../../../store.ts'
import { ChapterPaneMode } from '../ChapterPane.vue'
import { PhPalette } from '@phosphor-icons/vue'
import { computed, defineComponent, nextTick, PropType, ref, watch, watchEffect, useTemplateRef } from 'vue'

type State = DownloadTaskState | 'Idle'

const store = useStore()

const props = defineProps<{
  sortedGroups: [string, ChapterInfo[]][]
  reload: () => void
}>()

const chapterPaneMode = defineModel<ChapterPaneMode>('chapterPaneMode', { required: true })
// 当前tab的分组路径
const currentGroupPath = defineModel<string>('currentGroupPath', { required: true })
// 当前tab的分组
const currentGroup = computed<ChapterInfo[] | undefined>(
  () => props.sortedGroups.find(([path]) => path === currentGroupPath.value)?.[1],
)

const selectionOptions: PartialSelectionOptions = {
  selectables: '.selectable',
  features: { deselectOnBlur: true },
  boundaries: '.chapter-download-pane-selection-container',
}
// SelectionArea组件的ref
const selectionAreaRef = useTemplateRef('selectionAreaRef')
// 已勾选的章节id
const checkedIds = ref<Set<string>>(new Set())
// 已选中(被框选选到)的章节id
const selectedIds = ref<Set<string>>(new Set())
// 正在导出的章节id
const exportingChapterUuids = ref<Set<string>>(new Set())
// 如果漫画变了，清空勾选和选中状态
function clearCheckedAndSelected() {
  checkedIds.value.clear()
  selectedIds.value.clear()
  selectionAreaRef.value?.selection?.clearSelection()
}
watch(
  () => store.pickedComic,
  () => {
    clearCheckedAndSelected()
    exportingChapterUuids.value.clear()
  },
)

// 只保留selectable的章节
watchEffect(() => {
  if (store.pickedComic === undefined || props.sortedGroups === undefined) {
    return
  }

  const selectableChapterUuids = new Set(
    props.sortedGroups
      .flatMap(([, chapters]) => chapters)
      .filter((chapter) => isChapterSelectable(chapter))
      .map((chapter) => chapter.chapterUuid),
  )

  for (const uuid of checkedIds.value) {
    if (!selectableChapterUuids.has(uuid)) {
      checkedIds.value.delete(uuid)
    }
  }

  for (const uuid of selectedIds.value) {
    if (!selectableChapterUuids.has(uuid)) {
      selectedIds.value.delete(uuid)
    }
  }
})

// 提取章节id
function extractIds(elements: Element[]): string[] {
  return elements
    .map((element) => element.getAttribute('data-key'))
    .filter((uuid): uuid is string => uuid !== null)
    .filter((uuid) => currentGroup.value?.find((chapter) => chapter.chapterUuid === uuid) !== undefined)
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
  extractIds(added).forEach((uuid) => selectedIds.value.add(uuid))
  extractIds(removed).forEach((uuid) => selectedIds.value.delete(uuid))
}

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
        selectedIds.value.forEach((uuid) => checkedIds.value.add(uuid))
        dropdownShowing.value = false
      },
    },
  },
  {
    label: '取消勾选',
    key: 'uncheck',
    props: {
      onClick: () => {
        selectedIds.value.forEach((uuid) => checkedIds.value.delete(uuid))
        dropdownShowing.value = false
      },
    },
  },
  {
    label: '全选',
    key: 'check all',
    props: {
      onClick: () => {
        currentGroup.value?.filter((c) => isChapterSelectable(c)).forEach((c) => checkedIds.value.add(c.chapterUuid))
        dropdownShowing.value = false
      },
    },
  },
  {
    label: '取消全选',
    key: 'uncheck all',
    props: {
      onClick: () => {
        currentGroup.value?.forEach((chapter) => checkedIds.value.delete(chapter.chapterUuid))
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

async function exportPdf() {
  if (store.pickedComic === undefined) {
    return
  }

  const chapterUuids = currentGroup.value
    ?.filter((chapter) => isChapterSelectable(chapter) && checkedIds.value.has(chapter.chapterUuid))
    .map((chapter) => chapter.chapterUuid)
  if (chapterUuids === undefined || chapterUuids.length === 0) {
    return
  }

  store.progressesPaneTabName = 'export'
  chapterUuids.forEach((uuid) => exportingChapterUuids.value.add(uuid))

  const result = await commands.exportPdfChapters(store.pickedComic, chapterUuids)
  if (result.status === 'error') {
    console.error(result.error)
    exportingChapterUuids.value.clear()
    return
  }

  clearCheckedAndSelected()
  exportingChapterUuids.value.clear()
}

async function exportCbz() {
  if (store.pickedComic === undefined) {
    return
  }

  const chapterUuids = currentGroup.value
    ?.filter((chapter) => isChapterSelectable(chapter) && checkedIds.value.has(chapter.chapterUuid))
    .map((chapter) => chapter.chapterUuid)
  if (chapterUuids === undefined || chapterUuids.length === 0) {
    return
  }

  store.progressesPaneTabName = 'export'
  chapterUuids.forEach((uuid) => exportingChapterUuids.value.add(uuid))

  const result = await commands.exportCbzChapters(store.pickedComic, chapterUuids)
  if (result.status === 'error') {
    console.error(result.error)
    exportingChapterUuids.value.clear()
    return
  }

  clearCheckedAndSelected()
  exportingChapterUuids.value.clear()
}

function getChapterState(chapter: ChapterInfo): State {
  return store.progresses.get(chapter.chapterUuid)?.state ?? 'Idle'
}

function isDownloadingChapter(chapter: ChapterInfo) {
  const state = getChapterState(chapter)
  return state === 'Pending' || state === 'Downloading' || state === 'Paused'
}

function isDownloadedChapter(chapter: ChapterInfo): boolean {
  return chapter.isDownloaded === true
}

function isExportingChapter(chapter: ChapterInfo): boolean {
  return exportingChapterUuids.value.has(chapter.chapterUuid)
}

function isChapterSelectable(chapter: ChapterInfo): boolean {
  return !isDownloadingChapter(chapter) && isDownloadedChapter(chapter) && !isExportingChapter(chapter)
}

const ChapterCheckbox = defineComponent({
  name: 'ChapterCheckbox',
  props: {
    chapter: {
      type: Object as PropType<ChapterInfo>,
      required: true,
    },
  },
  setup: (props) => {
    return () => (
      <NCheckbox
        class={[
          'hover:bg-gray-200!',
          {
            selectable: isChapterSelectable(props.chapter),
            selected: selectedIds.value.has(props.chapter.chapterUuid),
            downloading: isDownloadingChapter(props.chapter),
            pdfExported: props.chapter.isPdfExported && !props.chapter.isCbzExported,
            cbzExported: props.chapter.isCbzExported && !props.chapter.isPdfExported,
            exportedBoth: props.chapter.isPdfExported && props.chapter.isCbzExported,
          },
        ]}
        checked={checkedIds.value.has(props.chapter.chapterUuid)}
        onUpdate:checked={(checked: boolean) => {
          if (checked) {
            checkedIds.value.add(props.chapter.chapterUuid)
          } else {
            checkedIds.value.delete(props.chapter.chapterUuid)
          }
        }}
        label={props.chapter.chapterTitle}
        disabled={!isChapterSelectable(props.chapter)}
      />
    )
  },
})
</script>

<template>
  <div v-if="store.pickedComic !== undefined" class="flex-1 flex flex-col overflow-auto">
    <div class="flex items-center select-none pt-2 gap-1 px-2">
      <n-radio-group v-model:value="chapterPaneMode" size="small">
        <n-radio-button value="download">下载</n-radio-button>
        <n-radio-button value="export">导出</n-radio-button>
      </n-radio-group>
      <n-popover placement="bottom" trigger="hover" raw>
        <template #trigger>
          <n-icon class="ml-1 cursor-help" size="22"><PhPalette /></n-icon>
        </template>
        <div class="flex flex-col gap-1 text-xs leading-5 bg-white p-2 rounded-lg">
          <div class="flex items-center gap-2">
            <span class="h-3.5 w-3.5 shrink-0 rounded border border-solid border-orange bg-orange-1" />
            <span>仅 曾导出过PDF</span>
          </div>
          <div class="flex items-center gap-2">
            <span class="h-3.5 w-3.5 shrink-0 rounded border border-solid border-fuchsia bg-fuchsia-1" />
            <span>仅 曾导出过CBZ</span>
          </div>
          <div class="flex items-center gap-2">
            <span class="h-3.5 w-3.5 shrink-0 rounded border border-solid border-indigo bg-indigo-2" />
            <span>曾导出过PDF+CBZ</span>
          </div>
        </div>
      </n-popover>
      <n-button class="ml-auto" size="small" @click="props.reload">刷新</n-button>
      <n-button size="small" type="primary" @click="exportCbz">导出cbz</n-button>
      <n-button size="small" type="primary" @click="exportPdf">导出pdf</n-button>
    </div>

    <SelectionArea ref="selectionAreaRef" :options="selectionOptions" @move="updateSelectedIds" @start="unselectAll" />

    <n-tabs class="flex-1 overflow-auto" v-model:value="currentGroupPath" type="line" size="small" animated>
      <n-tab-pane
        v-for="[groupPath, chapters] in sortedGroups"
        :key="groupPath"
        :name="groupPath"
        :tab="store.pickedComic.groups[groupPath].name"
        class="overflow-auto p-0! h-full">
        <div
          class="chapter-download-pane-selection-container box-border p-2 overflow-auto h-full"
          @contextmenu="showDropdown">
          <div class="grid grid-cols-3 gap-1.5 w-full">
            <ChapterCheckbox
              v-for="chapter in chapters"
              :key="chapter.chapterUuid"
              :data-key="chapter.chapterUuid"
              :chapter="chapter" />
          </div>
        </div>
      </n-tab-pane>
    </n-tabs>

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
.chapter-download-pane-selection-container {
  @apply select-none overflow-auto;
}

.chapter-download-pane-selection-container .pdfExported {
  @apply bg-orange-1;
}

.chapter-download-pane-selection-container .cbzExported {
  @apply bg-fuchsia-1;
}

.chapter-download-pane-selection-container .exportedBoth {
  @apply bg-indigo-2;
}

.chapter-download-pane-selection-container .selected {
  @apply bg-[rgb(204,232,255)] !important;
}

.chapter-download-pane-selection-container .downloading {
  @apply bg-[rgba(114,46,209,0.16)];
}

:deep(.n-checkbox__label) {
  @apply overflow-hidden whitespace-nowrap text-ellipsis;
}

:global(.selection-area) {
  @apply bg-[rgba(46,115,252,0.5)];
}
</style>
