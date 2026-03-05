<script setup lang="ts">
import { SelectionArea, SelectionEvent, SelectionOptions } from '@viselect/vue'
import { computed, nextTick, ref, watchEffect } from 'vue'
import { commands } from '../../../bindings.ts'
import { DropdownOption } from 'naive-ui'
import { useStore } from '../../../store.ts'
import { ChapterInfoWithState, ChapterPaneMode, State } from '../ChapterPane.vue'
import { PhPalette } from '@phosphor-icons/vue'

const props = defineProps<{
  sortedGroups: [string, ChapterInfoWithState[]][]
  reload: () => void
}>()

const chapterPaneMode = defineModel<ChapterPaneMode>('chapterPaneMode', { required: true })
const groupPath = defineModel<string>('groupPath', { required: true })

const store = useStore()

const selectionOptions = {
  selectables: '.selectable',
  features: { deselectOnBlur: true },
} as SelectionOptions

const checkedIds = ref<string[]>([])
const selectedIds = ref<Set<string>>(new Set())

const currentGroup = computed<ChapterInfoWithState[] | undefined>(
  () => props.sortedGroups.find(([path]) => path === groupPath.value)?.[1],
)

watchEffect(() => {
  const selectableChapterUuids = props.sortedGroups
    .flatMap(([, chapters]) => chapters)
    .filter((chapter) => isChapterSelectable(chapter))
    .map((chapter) => chapter.chapterUuid)
  checkedIds.value = checkedIds.value.filter((uuid) => selectableChapterUuids.includes(uuid))
})

function extractIds(elements: Element[]): string[] {
  return elements
    .map((element) => element.getAttribute('data-key'))
    .filter(Boolean)
    .filter((id) => {
      const chapterInfo = currentGroup.value?.find((chapter) => chapter.chapterUuid === id)
      return chapterInfo !== undefined && isChapterSelectable(chapterInfo)
    }) as string[]
}

function updateSelectedIds({
  store: {
    changed: { added, removed },
  },
}: SelectionEvent) {
  extractIds(added).forEach((id) => selectedIds.value.add(id))
  extractIds(removed).forEach((id) => selectedIds.value.delete(id))
}

function unselectAll({ event, selection }: SelectionEvent) {
  if (!event?.ctrlKey && !event?.metaKey) {
    selection.clearSelection()
    selectedIds.value.clear()
  }
}

const dropdownX = ref<number>(0)
const dropdownY = ref<number>(0)
const dropdownShowing = ref<boolean>(false)
const dropdownOptions = computed<DropdownOption[]>(() => [
  {
    label: '勾选',
    key: 'check',
    props: {
      onClick: () => {
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
          ?.filter((chapter) => isChapterSelectable(chapter) && !checkedIds.value.includes(chapter.chapterUuid))
          .forEach((chapter) => checkedIds.value.push(chapter.chapterUuid))
        dropdownShowing.value = false
      },
    },
  },
  {
    label: '取消全选',
    key: 'uncheck all',
    props: {
      onClick: () => {
        const currentGroupIds = currentGroup.value?.map((chapter) => chapter.chapterUuid) ?? []
        checkedIds.value = checkedIds.value.filter((id) => !currentGroupIds.includes(id))
        dropdownShowing.value = false
      },
    },
  },
])

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
    ?.filter((chapter) => isChapterSelectable(chapter) && checkedIds.value.includes(chapter.chapterUuid))
    .map((chapter) => chapter.chapterUuid)
  if (chapterUuids === undefined || chapterUuids.length === 0) {
    return
  }

  store.progressesPaneTabName = 'export'
  const result = await commands.exportPdfChapters(store.pickedComic, chapterUuids)
  if (result.status === 'error') {
    console.error(result.error)
    return
  }
}

async function exportCbz() {
  if (store.pickedComic === undefined) {
    return
  }

  const chapterUuids = currentGroup.value
    ?.filter((chapter) => isChapterSelectable(chapter) && checkedIds.value.includes(chapter.chapterUuid))
    .map((chapter) => chapter.chapterUuid)
  if (chapterUuids === undefined || chapterUuids.length === 0) {
    return
  }

  store.progressesPaneTabName = 'export'
  const result = await commands.exportCbzChapters(store.pickedComic, chapterUuids)
  if (result.status === 'error') {
    console.error(result.error)
    return
  }
}

function isDownloading(state: State) {
  return state === 'Pending' || state === 'Downloading' || state === 'Paused'
}

function isChapterSelectable(chapter: ChapterInfoWithState): boolean {
  return !isDownloading(chapter.state) && chapter.isDownloaded === true
}
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
          <n-icon class="ml-1 cursor-help" size="22"><PhPalette class="" /></n-icon>
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

    <n-tabs class="flex-1 overflow-auto" v-model:value="groupPath" type="line" size="small" animated>
      <n-tab-pane
        v-for="[groupPath, chapters] in sortedGroups"
        :key="groupPath"
        :name="groupPath"
        :tab="store.pickedComic.groups[groupPath].name"
        class="overflow-auto p-0! h-full">
        <SelectionArea
          class="selection-container flex flex-col flex-1 box-border pt-2 px-2 overflow-auto h-full"
          :options="selectionOptions"
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
              :class="{
                selected: selectedIds.has(chapter.chapterUuid),
                downloading: isDownloading(chapter.state),
                pdfExported: chapter.isPdfExported && !chapter.isCbzExported,
                cbzExported: chapter.isCbzExported && !chapter.isPdfExported,
                exportedBoth: chapter.isPdfExported && chapter.isCbzExported,
              }" />
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
      :on-clickoutside="() => (dropdownShowing = false)" />
  </div>
</template>

<style scoped>
.selection-container {
  @apply select-none overflow-auto;
}

.selection-container .pdfExported {
  @apply bg-orange-1;
}

.selection-container .cbzExported {
  @apply bg-fuchsia-1;
}

.selection-container .exportedBoth {
  @apply bg-indigo-2;
}

.selection-container .selected {
  @apply bg-[rgb(204,232,255)];
}

.selection-container .downloading {
  @apply bg-[rgba(114,46,209,0.16)];
}

:deep(.n-checkbox__label) {
  @apply overflow-hidden whitespace-nowrap text-ellipsis;
}

:global(.selection-area) {
  @apply bg-[rgba(46,115,252,0.5)];
}
</style>
