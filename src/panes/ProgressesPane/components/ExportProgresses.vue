<script setup lang="tsx">
import { events, commands } from '../../../bindings.ts'
import { onMounted, ref, watchEffect, nextTick, onUnmounted, defineComponent, PropType, computed } from 'vue'
import { NIcon, DropdownOption, NDropdown, NProgress } from 'naive-ui'
import { PhChecks, PhCircleNotch, PhFolderOpen, PhTrash } from '@phosphor-icons/vue'
import { PartialSelectionOptions, SelectionArea, SelectionEvent } from '@viselect/vue'
import { useStore } from '../../../store.ts'
import IconButton from '../../../components/IconButton.vue'

type ProgressState = 'Processing' | 'Error' | 'End'

export interface ProgressData {
  uuid: string
  exportType: 'cbz' | 'pdf'
  state: ProgressState
  comicTitle: string
  current: number
  total: number
  percentage: number
  indicator: string
  chapterExportDir?: string
  comicPathWord?: string
}

const store = useStore()
const selectionOptions: PartialSelectionOptions = {
  selectables: '.selectable',
  features: { deselectOnBlur: true },
  boundaries: '.export-progresses-selection-container',
}
const selectedIds = ref<Set<string>>(new Set())
const { dropdownX, dropdownY, dropdownShowing, dropdownOptions, showDropdown } = useDropdown()

const progresses = ref<Map<string, ProgressData>>(new Map())

watchEffect(() => {
  // 保证selectedIds中的uuid在progresses中存在
  const uuids = new Set(progresses.value.keys())
  for (const uuid of selectedIds.value) {
    if (!uuids.has(uuid)) {
      selectedIds.value.delete(uuid)
    }
  }
})

async function syncPickedAndDownloadedComic(comicPathWord: string) {
  const pickedComic = store.pickedComic?.comic.path_word === comicPathWord ? store.pickedComic : undefined
  const downloadedComic = store.downloadedComics.find((comic) => comic.comic.path_word === comicPathWord)

  if (pickedComic === undefined && downloadedComic === undefined) {
    return
  }

  const comic = pickedComic ?? downloadedComic
  if (comic === undefined) {
    return
  }

  const result = await commands.getSyncedComic(comic)
  if (result.status !== 'ok') {
    return
  }

  if (pickedComic !== undefined) {
    Object.assign(pickedComic, result.data)
  }
  if (downloadedComic !== undefined) {
    Object.assign(downloadedComic, result.data)
  }
}

let unListenExportCbzEvent: () => void | undefined
let unListenExportPdfEvent: () => void | undefined
// 监听导出事件
onMounted(() => {
  // 处理导出CBZ事件
  events.exportCbzEvent
    .listen(async ({ payload: exportEvent }) => {
      if (exportEvent.event === 'Start') {
        const { uuid, comicTitle, total } = exportEvent.data
        progresses.value.set(uuid, {
          uuid,
          exportType: 'cbz',
          state: 'Processing',
          comicTitle,
          current: 0,
          total,
          percentage: 0,
          indicator: 'CBZ创建CBZ中',
        })
      } else if (exportEvent.event === 'Progress') {
        const { uuid, current } = exportEvent.data
        const progressData = progresses.value.get(uuid)
        if (progressData !== undefined) {
          progressData.state = 'Processing'
          progressData.current = current
          progressData.percentage = (current / progressData.total) * 100
          progressData.indicator = `CBZ创建中 ${current}/${progressData.total}`
        }
      } else if (exportEvent.event === 'Error') {
        const { uuid } = exportEvent.data
        const progressData = progresses.value.get(uuid)
        if (progressData !== undefined) {
          progressData.state = 'Error'
          progressData.indicator = 'CBZ创建失败'
        }
      } else if (exportEvent.event === 'End') {
        const { uuid, comicPathWord, chapterExportDir } = exportEvent.data
        const progressData = progresses.value.get(uuid)
        if (progressData !== undefined) {
          progressData.state = 'End'
          progressData.chapterExportDir = chapterExportDir
          progressData.comicPathWord = comicPathWord
          progressData.indicator = 'CBZ创建完成'
        }
        // 同步当前漫画数据
        await syncPickedAndDownloadedComic(comicPathWord)
      }
    })
    .then((unListenFn) => {
      unListenExportCbzEvent = unListenFn
    })

  // 处理导出PDF事件
  events.exportPdfEvent
    .listen(async ({ payload: exportEvent }) => {
      if (exportEvent.event === 'CreateStart') {
        const { uuid, comicTitle, total } = exportEvent.data
        progresses.value.set(uuid, {
          uuid,
          exportType: 'pdf',
          state: 'Processing',
          comicTitle,
          current: 0,
          total,
          percentage: 0,
          indicator: 'PDF创建中',
        })
      } else if (exportEvent.event === 'CreateProgress') {
        const { uuid, current } = exportEvent.data
        const progressData = progresses.value.get(uuid)
        if (progressData !== undefined) {
          progressData.state = 'Processing'
          progressData.current = current
          progressData.percentage = (current / progressData.total) * 100
          progressData.indicator = `PDF创建中 ${current}/${progressData.total}`
        }
      } else if (exportEvent.event === 'CreateError') {
        const { uuid } = exportEvent.data
        const progressData = progresses.value.get(uuid)
        if (progressData !== undefined) {
          progressData.state = 'Error'
          progressData.indicator = '创建PDF失败'
        }
      } else if (exportEvent.event === 'CreateEnd') {
        const { uuid, comicPathWord, chapterExportDir } = exportEvent.data
        const progressData = progresses.value.get(uuid)
        if (progressData !== undefined) {
          progressData.state = 'End'
          progressData.chapterExportDir = chapterExportDir
          progressData.comicPathWord = comicPathWord
          progressData.indicator = 'PDF创建完成'
        }
        // 同步当前漫画数据
        await syncPickedAndDownloadedComic(comicPathWord)
      } else if (exportEvent.event === 'MergeStart') {
        const { uuid, comicTitle } = exportEvent.data
        progresses.value.set(uuid, {
          uuid,
          exportType: 'pdf',
          state: 'Processing',
          comicTitle,
          current: 0,
          total: 1,
          percentage: 0,
          indicator: 'PDF合并中',
        })
      } else if (exportEvent.event === 'MergeError') {
        const { uuid } = exportEvent.data
        const progressData = progresses.value.get(uuid)
        if (progressData !== undefined) {
          progressData.state = 'Error'
          progressData.indicator = 'PDF合并失败'
        }
      } else if (exportEvent.event === 'MergeEnd') {
        const { uuid, comicPathWord, chapterExportDir } = exportEvent.data
        const progressData = progresses.value.get(uuid)
        if (progressData !== undefined) {
          progressData.state = 'End'
          progressData.current = progressData.total
          progressData.percentage = 100
          progressData.chapterExportDir = chapterExportDir
          progressData.comicPathWord = comicPathWord
          progressData.indicator = 'PDF合并完成'
        }
      }
    })
    .then((unListenFn) => {
      unListenExportPdfEvent = unListenFn
    })
})

onUnmounted(() => {
  unListenExportCbzEvent?.()
  unListenExportPdfEvent?.()
})

function extractIds(elements: Element[]): string[] {
  return elements
    .map((element) => element.getAttribute('data-key'))
    .filter(Boolean)
    .filter((uuid) => uuid !== null)
}

function updateSelectedIds({
  store: {
    changed: { added, removed },
  },
}: SelectionEvent) {
  extractIds(added).forEach((uuid) => selectedIds.value.add(uuid))
  extractIds(removed).forEach((uuid) => selectedIds.value.delete(uuid))
}

function unselectAll({ event, selection }: SelectionEvent) {
  if (!event?.ctrlKey && !event?.metaKey) {
    selection.clearSelection()
    selectedIds.value.clear()
  }
}

function useDropdown() {
  const dropdownX = ref<number>(0)
  const dropdownY = ref<number>(0)
  const dropdownShowing = ref<boolean>(false)
  const dropdownOptions: DropdownOption[] = [
    {
      label: '全选',
      key: 'check all',
      icon: () => (
        <NIcon size="20">
          <PhChecks />
        </NIcon>
      ),
      props: {
        onClick: () => {
          progresses.value.forEach((p, uuid) => {
            if (p.state !== 'Processing') {
              selectedIds.value.add(uuid)
            }
          })
          dropdownShowing.value = false
        },
      },
    },
    {
      label: '删除',
      key: 'delete',
      icon: () => (
        <NIcon size="20">
          <PhTrash />
        </NIcon>
      ),
      props: {
        onClick: () => {
          selectedIds.value.forEach((uuid) => progresses.value.delete(uuid))
          dropdownShowing.value = false
        },
      },
    },
  ]

  async function showDropdown(e: MouseEvent) {
    dropdownShowing.value = false
    await nextTick()
    dropdownShowing.value = true
    dropdownX.value = e.clientX
    dropdownY.value = e.clientY
  }

  return {
    dropdownX,
    dropdownY,
    dropdownShowing,
    dropdownOptions,
    showDropdown,
  }
}

const ExportProgress = defineComponent({
  name: 'ExportProgress',
  props: {
    uuid: {
      type: String,
      required: true,
    },
    p: {
      type: Object as PropType<ProgressData>,
      required: true,
    },
  },
  setup(props) {
    const progressSelectable = computed(() => props.p.state !== 'Processing')
    const selectableClass = computed(() => {
      return ['selectable', selectedIds.value.has(props.uuid) ? 'selected shadow-md' : 'hover:bg-gray-1']
    })

    function onContextMenu() {
      if (selectedIds.value.has(props.p.uuid)) {
        return
      }
      selectedIds.value.clear()
      selectedIds.value.add(props.p.uuid)
    }

    async function showChapterExportDirInFileManager() {
      if (props.p.chapterExportDir === undefined) {
        return
      }

      const result = await commands.showPathInFileManager(props.p.chapterExportDir)
      if (result.status === 'error') {
        console.error(result.error)
      }
    }

    return () => (
      <div
        data-key={props.uuid}
        class={[
          'flex flex-col border border-solid rounded-md border-gray-2 p-1 mb-2',
          progressSelectable.value && selectableClass.value,
        ]}
        onContextmenu={progressSelectable.value ? onContextMenu : undefined}>
        <div class="text-ellipsis whitespace-nowrap overflow-hidden" title={props.p.comicTitle}>
          {props.p.comicTitle}
        </div>

        {props.p.state === 'Processing' && (
          <div class="flex">
            <NIcon class="text-blue-5 mr-2" size={20}>
              <PhCircleNotch class="animate-spin" />
            </NIcon>
            <NProgress class="text-blue-5" percentage={props.p.percentage} processing>
              {props.p.indicator}
            </NProgress>
          </div>
        )}
        {props.p.state === 'Error' && (
          <NProgress class="text-red-5" status="error" percentage={props.p.percentage}>
            {props.p.indicator}
          </NProgress>
        )}
        {props.p.state === 'End' && (
          <div class="text-green-5 flex items-center ml-auto">
            <span>{props.p.indicator}</span>
          </div>
        )}
        {props.p.chapterExportDir !== undefined && (
          <IconButton class="ml-auto" title={'打开导出目录'} onClick={showChapterExportDirInFileManager}>
            <PhFolderOpen size={24} />
          </IconButton>
        )}
      </div>
    )
  },
})
</script>

<template>
  <div class="h-full export-progresses-selection-container px-2" @contextmenu="showDropdown">
    <SelectionArea :options="selectionOptions" @move="updateSelectedIds" @start="unselectAll" />
    <div class="flex flex-col">
      <div class="flex">
        <span class="ml-auto animate-pulse text-blue">左键拖动进行框选，右键打开菜单</span>
      </div>

      <ExportProgress v-for="[uuid, p] in progresses" :key="uuid" :p="p" :uuid="uuid" />
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
.export-progresses-selection-container {
  @apply select-none overflow-auto;
}

.export-progresses-selection-container .selected {
  @apply bg-[rgb(204,232,255)];
}
</style>
