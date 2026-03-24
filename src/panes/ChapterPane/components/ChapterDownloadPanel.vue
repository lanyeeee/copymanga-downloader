<script setup lang="ts">
import { SelectionArea, SelectionEvent, SelectionOptions } from '@viselect/vue'
import { computed, nextTick, ref, watchEffect } from 'vue'
import { commands } from '../../../bindings.ts'
import {
  DropdownOption,
  NButton,
  NCheckbox,
  NCheckboxGroup,
  NDropdown,
  NRadioButton,
  NRadioGroup,
  NTabPane,
  NTabs,
} from 'naive-ui'
import { useStore } from '../../../store.ts'
import { ChapterInfoWithState, ChapterPaneMode, State } from '../ChapterPane.vue'

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

async function downloadChapters() {
  if (store.pickedComic === undefined) {
    return
  }

  const chapterUuidsToDownload = currentGroup.value
    ?.filter((chapter) => isChapterSelectable(chapter) && checkedIds.value.includes(chapter.chapterUuid))
    .map((chapter) => chapter.chapterUuid)
  if (chapterUuidsToDownload === undefined || chapterUuidsToDownload.length === 0) {
    return
  }

  for (const chapterUuid of chapterUuidsToDownload) {
    await commands.createDownloadTask(store.pickedComic, chapterUuid)
  }
}

function isDownloading(state: State) {
  return state === 'Pending' || state === 'Downloading' || state === 'Paused'
}

function isChapterSelectable(chapter: ChapterInfoWithState): boolean {
  return !isDownloading(chapter.state) && chapter.isDownloaded !== true
}
</script>

<template>
  <div v-if="store.pickedComic !== undefined" class="flex-1 flex flex-col overflow-auto">
    <div class="flex items-center select-none pt-2 gap-1 px-2">
      <n-radio-group v-model:value="chapterPaneMode" size="small">
        <n-radio-button value="download">下载</n-radio-button>
        <n-radio-button value="export">导出</n-radio-button>
      </n-radio-group>
      <n-button class="ml-auto" size="small" @click="props.reload">刷新</n-button>
      <n-button size="small" type="primary" @click="downloadChapters">下载勾选章节</n-button>
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
                downloaded: chapter.isDownloaded,
                downloading: !chapter.isDownloaded && isDownloading(chapter.state),
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

.selection-container .selected {
  @apply bg-[rgb(204,232,255)] !important;
}

.selection-container .downloaded {
  @apply bg-[rgba(24,160,88,0.16)];
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
