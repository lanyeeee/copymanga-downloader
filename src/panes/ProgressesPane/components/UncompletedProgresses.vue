<script setup lang="tsx">
import { ProgressData } from '../../../types.ts'
import { ref, watchEffect, computed, nextTick } from 'vue'
import { SelectionArea, SelectionEvent } from '@viselect/vue'
import { commands, DownloadTaskState } from '../../../bindings.ts'
import { DropdownOption, NIcon, ProgressProps } from 'naive-ui'
import { useStore } from '../../../store.ts'
import {
  PhPause,
  PhChecks,
  PhTrash,
  PhCaretRight,
  PhCloudArrowDown,
  PhClock,
  PhWarningCircle,
} from '@phosphor-icons/vue'

const store = useStore()

const selectedIds = ref<Set<string>>(new Set())
const selectionAreaRef = ref<InstanceType<typeof SelectionArea>>()
const selectableRefs = ref<HTMLDivElement[]>([])
const { dropdownX, dropdownY, dropdownShowing, dropdownOptions, showDropdown } = useDropdown()

const uncompletedProgresses = computed<[string, ProgressData][]>(() =>
  Array.from(store.progresses.entries())
    .filter(([, { state }]) => state !== 'Completed' && state !== 'Cancelled')
    .sort((a, b) => b[1].totalImgCount - a[1].totalImgCount),
)

watchEffect(() => {
  // 只保留未完成的章节id
  const uncompletedIds = new Set(uncompletedProgresses.value.map(([chapterUuid]) => chapterUuid))
  selectedIds.value = new Set([...selectedIds.value].filter((chapterUuid) => uncompletedIds.has(chapterUuid)))
})

function extractIds(elements: Element[]): string[] {
  return elements
    .map((element) => element.getAttribute('data-key'))
    .filter(Boolean)
    .filter((id) => id !== null)
}

function updateSelectedIds({
  store: {
    changed: { added, removed },
  },
}: SelectionEvent) {
  extractIds(added).forEach((chapterUuid) => selectedIds.value.add(chapterUuid))
  extractIds(removed).forEach((chapterUuid) => selectedIds.value.delete(chapterUuid))
}

function unselectAll({ event, selection }: SelectionEvent) {
  if (!event?.ctrlKey && !event?.metaKey) {
    selection.clearSelection()
    selectedIds.value.clear()
  }
}

async function handleProgressDoubleClick(state: DownloadTaskState, chapterUuid: string) {
  if (state === 'Downloading' || state === 'Pending') {
    const result = await commands.pauseDownloadTask(chapterUuid)
    if (result.status === 'error') {
      console.error(result.error)
    }
  } else if (state === 'Paused') {
    const result = await commands.resumeDownloadTask(chapterUuid)
    if (result.status === 'error') {
      console.error(result.error)
    }
  } else {
    const progressData = store.progresses.get(chapterUuid)
    if (progressData === undefined) {
      return
    }
    const { comic } = progressData
    const result = await commands.createDownloadTask(comic, chapterUuid)
    if (result.status === 'error') {
      console.error(result.error)
    }
  }
}

function handleProgressContextMenu(chapterUuid: string) {
  if (selectedIds.value.has(chapterUuid)) {
    return
  }
  selectedIds.value.clear()
  selectedIds.value.add(chapterUuid)
}

function useDropdown() {
  const dropdownX = ref<number>(0)
  const dropdownY = ref<number>(0)
  const dropdownShowing = ref<boolean>(false)
  const dropdownOptions: DropdownOption[] = [
    {
      label: '全选',
      key: 'select-all',
      icon: () => (
        <NIcon size="20">
          <PhChecks />
        </NIcon>
      ),
      props: {
        onClick: () => {
          if (selectionAreaRef.value === undefined) {
            return
          }
          const selection = selectionAreaRef.value.selection
          if (selection === undefined) {
            return
          }
          selection.select(selectableRefs.value)
          dropdownShowing.value = false
        },
      },
    },
    {
      label: '继续',
      key: 'resume',
      icon: () => (
        <NIcon size="20">
          <PhCaretRight />
        </NIcon>
      ),
      props: {
        onClick: () => {
          selectedIds.value.forEach(async (chapterUuid) => {
            const progressData = store.progresses.get(chapterUuid)
            if (progressData === undefined) {
              return
            }
            const { state, comic } = progressData
            if (state === 'Cancelled' || state === 'Completed' || state === 'Failed') {
              const result = await commands.createDownloadTask(comic, chapterUuid)
              if (result.status === 'error') {
                console.error(result.error)
              }
              return
            }

            const result = await commands.resumeDownloadTask(chapterUuid)
            if (result.status === 'error') {
              console.error(result.error)
            }
          })
          dropdownShowing.value = false
        },
      },
    },
    {
      label: '暂停',
      key: 'pause',
      icon: () => (
        <NIcon size="20">
          <PhPause />
        </NIcon>
      ),
      props: {
        onClick: () => {
          selectedIds.value.forEach(async (chapterUuid) => {
            const progressData = store.progresses.get(chapterUuid)
            if (progressData === undefined) {
              return
            }
            const { state } = progressData
            if (state === 'Cancelled' || state === 'Completed' || state === 'Failed') {
              return
            }

            const result = await commands.pauseDownloadTask(chapterUuid)
            if (result.status === 'error') {
              console.error(result.error)
            }
          })
          dropdownShowing.value = false
        },
      },
    },
    {
      label: '取消',
      key: 'cancel',
      icon: () => (
        <NIcon size="20">
          <PhTrash />
        </NIcon>
      ),
      props: {
        onClick: () => {
          selectedIds.value.forEach(async (chapterUuid) => {
            const progressData = store.progresses.get(chapterUuid)
            if (progressData === undefined) {
              return
            }
            const { state } = progressData
            if (state === 'Cancelled' || state === 'Completed' || state === 'Failed') {
              return
            }

            const result = await commands.cancelDownloadTask(chapterUuid)
            if (result.status === 'error') {
              console.error(result.error)
            }
          })
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

function stateToStatus(state: DownloadTaskState): ProgressProps['status'] {
  if (state === 'Completed') {
    return 'success'
  } else if (state === 'Paused') {
    return 'warning'
  } else if (state === 'Failed') {
    return 'error'
  } else {
    return 'default'
  }
}

function stateToColorClass(state: DownloadTaskState) {
  if (state === 'Downloading') {
    return 'text-blue-500'
  } else if (state === 'Pending') {
    return 'text-gray-500'
  } else if (state === 'Paused') {
    return 'text-yellow-500'
  } else if (state === 'Failed') {
    return 'text-red-500'
  } else if (state === 'Completed') {
    return 'text-green-500'
  } else if (state === 'Cancelled') {
    return 'text-stone-500'
  }

  return ''
}
</script>

<template>
  <SelectionArea
    ref="selectionAreaRef"
    class="h-full flex flex-col selection-container px-2"
    :options="{ selectables: '.selectable', features: { deselectOnBlur: true } }"
    @contextmenu="showDropdown"
    @move="updateSelectedIds"
    @start="unselectAll">
    <span class="ml-auto select-none">左键拖动进行框选，右键打开菜单，双击暂停/继续</span>
    <div class="h-full select-none">
      <div
        v-for="[
          chapterUuid,
          { comic, chapterInfo, state, percentage, totalImgCount, retryAfter, indicator },
        ] in uncompletedProgresses"
        :key="chapterUuid"
        ref="selectableRefs"
        :data-key="chapterUuid"
        :class="[
          'selectable p-3 mb-2 rounded-lg',
          selectedIds.has(chapterUuid) ? 'selected shadow-md' : 'hover:bg-gray-1',
        ]"
        @dblclick="() => handleProgressDoubleClick(state, chapterUuid)"
        @contextmenu="() => handleProgressContextMenu(chapterUuid)">
        <div class="grid grid-cols-[1fr_1fr]">
          <div class="text-ellipsis whitespace-nowrap overflow-hidden" :title="comic.comic.name">
            {{ comic.comic.name }}
          </div>
          <div class="text-ellipsis whitespace-nowrap overflow-hidden" :title="chapterInfo.chapterTitle">
            {{ chapterInfo.chapterTitle }}
          </div>
        </div>
        <div class="flex">
          <n-icon :class="[stateToColorClass(state), 'mr-2']" :size="20">
            <PhCloudArrowDown v-if="state === 'Downloading'" />
            <PhClock v-else-if="state === 'Pending'" />
            <PhPause v-else-if="state === 'Paused'" />
            <PhWarningCircle v-else-if="state === 'Failed'" />
          </n-icon>
          <div v-if="retryAfter !== 0">{{ indicator }}</div>
          <div v-else-if="totalImgCount === 0" class="ml-auto">{{ indicator }}</div>
          <n-progress
            v-else
            :class="stateToColorClass(state)"
            :status="stateToStatus(state)"
            :percentage="percentage"
            :processing="state === 'Downloading'">
            {{ indicator }}
          </n-progress>
        </div>
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
  </SelectionArea>
</template>

<style scoped>
.selection-container {
  @apply select-none overflow-auto;
}

.selection-container .selected {
  @apply bg-[rgb(204,232,255)];
}

:global(.selection-area) {
  @apply bg-[rgba(46,115,252,0.5)];
}
</style>
