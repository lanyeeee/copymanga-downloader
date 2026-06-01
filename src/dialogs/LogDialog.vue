<script setup lang="tsx">
import { commands, events, JsonValue, LogLevel, LogMetadata } from '../bindings.ts'
import {
  NButton,
  NCheckbox,
  NDialog,
  NInput,
  NInputGroup,
  NModal,
  NSelect,
  NTag,
  useNotification,
  NIcon,
} from 'naive-ui'
import {
  computed,
  defineComponent,
  nextTick,
  onMounted,
  onUnmounted,
  PropType,
  ref,
  shallowRef,
  triggerRef,
  useTemplateRef,
  watch,
} from 'vue'
import { appDataDir, basename } from '@tauri-apps/api/path'
import { path } from '@tauri-apps/api'
import { useStore } from '../store.ts'
import { open } from '@tauri-apps/plugin-dialog'
import { VList } from 'virtua/vue'
import { PhArrowDown, PhArrowUp } from '@phosphor-icons/vue'

export type LogField = {
  key: string
  value: string
}

type LogRecord = LogMetadata & {
  id: number
  textForFilter: string
  renderData: {
    message: string
    extraFields: LogField[]
    spanLines?: Array<{
      name: string
      args: LogField[]
    }>
  }
}

const store = useStore()

const notification = useNotification()

const showing = defineModel<boolean>('showing', { required: true })

let nextLogRecordId = 1

const logLevelOptions = [
  { value: 'TRACE', label: 'TRACE' },
  { value: 'DEBUG', label: 'DEBUG' },
  { value: 'INFO', label: 'INFO' },
  { value: 'WARN', label: 'WARN' },
  { value: 'ERROR', label: 'ERROR' },
]

const vListRef = useTemplateRef('vListRef')
const isAtTop = ref<boolean>(false)
const isAtBottom = ref<boolean>(false)

const viewMode = ref<'live' | 'file'>('live')
const currentFileName = ref<string>('')

const liveLogRecords = shallowRef<LogRecord[]>([])
const fileLogRecords = shallowRef<LogRecord[]>([])

const filterText = ref<string>('')
const selectedLevel = ref<LogLevel>('INFO')
const logsDirSize = ref<number>(0)

const formatedLogsDirSize = computed<string>(() => {
  const units = ['B', 'KB', 'MB']
  let size = logsDirSize.value
  let unitIndex = 0

  while (size >= 1024 && unitIndex < 2) {
    size /= 1024
    unitIndex++
  }

  // 保留两位小数
  return `${size.toFixed(2)} ${units[unitIndex]}`
})

const filteredLogs = computed<LogRecord[]>(() => {
  // 根据模式选择数据源
  const sourceRecords = viewMode.value === 'live' ? liveLogRecords.value : fileLogRecords.value

  return sourceRecords.filter(({ level, textForFilter }) => {
    const logLevelPriority = {
      TRACE: 0,
      DEBUG: 1,
      INFO: 2,
      WARN: 3,
      ERROR: 4,
    }
    // 首先按日志等级筛选
    if (logLevelPriority[level] < logLevelPriority[selectedLevel.value]) {
      return false
    }
    // 然后按过滤文本筛选
    if (filterText.value === '') {
      return true
    }

    return textForFilter.toLowerCase().includes(filterText.value.toLowerCase())
  })
})

onMounted(async () => {
  const result = await commands.getLogsDirSize()
  if (result.status === 'error') {
    console.error(result.error)
    return
  }
  // 检查日志目录大小
  if (result.data > 50 * 1024 * 1024) {
    notification.warning({
      title: '日志目录大小超过50MB，请及时清理日志文件',
      description: () => (
        <>
          <div>
            点击右上角的 <span class="bg-gray-2 px-1">日志</span> 按钮
          </div>
          <div>
            里边有 <span class="bg-gray-2 px-1">打开日志目录</span> 按钮
          </div>
          <div>
            你也可以在里边取消勾选 <span class="bg-gray-2 px-1">输出文件日志</span>
          </div>
          <div>这样将不再产生文件日志</div>
        </>
      ),
    })
  }
})

watch(filteredLogs, async () => {
  await nextTick()
  updateScrollEdgeState()
})

watch(showing, async () => {
  if (showing.value) {
    const result = await commands.getLogsDirSize()
    if (result.status === 'error') {
      console.error(result.error)
      return
    }
    logsDirSize.value = result.data
  }
})

let unListenLogEvent: () => void | undefined
onMounted(() => {
  events.logEvent
    .listen(({ payload: logEvent }) => {
      const logMetadata: LogMetadata = JSON.parse(logEvent.jsonRaw)

      const logRecord = logMetadataToLogRecord(logMetadata)
      liveLogRecords.value.push(logRecord)
      triggerRef(liveLogRecords)

      if (logRecord.level === 'ERROR') {
        notification.error({
          title: (logRecord.fields['err_title'] as string) || 'Error',
          description: (logRecord.fields['message'] as string) || 'Unknown Error',
          duration: 0,
        })
      }
    })
    .then((unListenFn) => {
      unListenLogEvent = unListenFn
    })
})
onUnmounted(() => {
  unListenLogEvent?.()
})

function formatJsonValue(jsonValue: JsonValue): string {
  if (Array.isArray(jsonValue)) return `[${jsonValue.map(formatJsonValue).join(', ')}]`
  if (typeof jsonValue === 'object' && jsonValue !== null)
    return `{${Object.entries(jsonValue)
      .map(([k, v]) => `${k}: ${formatJsonValue(v)}`)
      .join(', ')}}`
  return typeof jsonValue === 'string' ? `"${jsonValue}"` : String(jsonValue)
}

function logMetadataToLogRecord(meta: LogMetadata): LogRecord {
  const message = meta.fields['message'] as string

  const extraFields = Object.entries(meta.fields)
    .filter(([key]) => key !== 'message')
    .map(([key, jsonValue]) => ({
      key,
      value: formatJsonValue(jsonValue),
    }))

  const spanLines = meta.spans
    ?.slice()
    .reverse()
    .map((span) => {
      const args = Object.entries(span)
        .filter(([key]) => key !== 'name')
        .map(([key, jsonValue]) => ({
          key,
          value: formatJsonValue(jsonValue),
        }))
      return { name: span.name, args }
    })

  const extraFieldsStr = extraFields.map((f) => `${f.key}: ${f.value}`).join(', ')
  const headerLine = `${meta.timestamp} ${meta.level} ${meta.target}: ${message} ${extraFieldsStr}`

  const locationLine = `at ${meta.filename}:${meta.line_number}`

  const contextLines = spanLines
    ?.map((s) => {
      const argsStr = s.args.map((a) => `${a.key}: ${a.value}`).join(', ')
      return `in ${s.name} ${argsStr}`
    })
    .join('\n')

  const textForFilter = `${headerLine}\n${locationLine}\n${contextLines}`

  return {
    ...meta,
    id: nextLogRecordId++,
    textForFilter,
    renderData: { message, extraFields, spanLines },
  }
}

function clearLiveLogRecords() {
  liveLogRecords.value = []
}

async function showLogsDirInFileManager() {
  const logsDir = await path.join(await appDataDir(), '日志')
  const result = await commands.showPathInFileManager(logsDir)
  if (result.status === 'error') {
    console.error(result.error)
  }
}

async function openLogFile() {
  const logsDir = await path.join(await appDataDir(), '日志')

  const selectedFilePath = await open({
    defaultPath: logsDir,
    filters: [{ name: 'Log Files', extensions: ['log'] }],
  })

  if (selectedFilePath === null) {
    return
  }

  const result = await commands.openLogFile(selectedFilePath)
  if (result.status === 'error') {
    console.error(result.error)
    return
  }

  fileLogRecords.value = result.data.map(logMetadataToLogRecord)
  currentFileName.value = await basename(selectedFilePath)

  viewMode.value = 'file'
}

function exitFileMode() {
  viewMode.value = 'live'
  currentFileName.value = ''
  fileLogRecords.value = []
}

function jumpToTop() {
  vListRef.value?.scrollTo(0)
}

function jumpToBottom() {
  vListRef.value?.scrollToIndex(filteredLogs.value.length - 1)
}

function updateScrollEdgeState() {
  if (vListRef.value === null) {
    return
  }

  const { scrollOffset, scrollSize, viewportSize } = vListRef.value

  const threshold = 50

  isAtTop.value = scrollOffset <= threshold

  isAtBottom.value = scrollOffset + viewportSize >= scrollSize - threshold
}

const LogRecordComponent = defineComponent({
  name: 'LogRecordComponent',
  props: {
    logRecord: {
      type: Object as PropType<LogRecord>,
      required: true,
    },
  },
  setup(props) {
    const levelTextClass = computed(() => {
      switch (props.logRecord.level) {
        case 'TRACE':
          return 'text-fuchsia-400'
        case 'DEBUG':
          return 'text-blue-400'
        case 'INFO':
          return 'text-green-400'
        case 'WARN':
          return 'text-amber-400'
        case 'ERROR':
          return 'text-red-400'
        default:
          return ''
      }
    })

    const levelBoldClass = computed(() => {
      switch (props.logRecord.level) {
        case 'TRACE':
          return 'font-bold text-fuchsia-600'
        case 'DEBUG':
          return 'font-bold text-blue-600'
        case 'INFO':
          return 'font-bold text-green-600'
        case 'WARN':
          return 'font-bold text-amber-600'
        case 'ERROR':
          return 'font-bold text-red-600'
        default:
          return ''
      }
    })

    const levelTagClass = computed(() => {
      switch (props.logRecord.level) {
        case 'TRACE':
          return 'rounded-md px-1 py-0.5 bg-fuchsia-500/20 text-fuchsia-300 border-solid border-2 border-fuchsia-500/30'
        case 'DEBUG':
          return 'rounded-md px-1 py-0.5 bg-blue-500/20 text-blue-300 border-solid border-2 border-blue-500/30'
        case 'INFO':
          return 'rounded-md px-1 py-0.5 bg-green-500/20 text-green-300 border-solid border-2 border-green-500/30'
        case 'WARN':
          return 'rounded-md px-1 py-0.5 bg-amber-500/20 text-amber-300 border-solid border-2 border-amber-500/30'
        case 'ERROR':
          return 'rounded-md px-1 py-0.5 bg-red-500/20 text-red-300 border-solid border-2 border-red-500/30'
        default:
          return ''
      }
    })

    return () => (
      <div class="py-1 px-3 hover:bg-white/5 whitespace-pre-wrap break-all">
        <div>
          <span class="text-gray-500 whitespace-nowrap">{props.logRecord.timestamp}</span>
          <span> </span>
          <span class={levelTextClass.value}>
            <span class={levelTagClass.value}>{props.logRecord.level}</span>
            <span> </span>
            <span class={levelBoldClass.value}>{props.logRecord.target}:</span>
            <span> </span>
            <span>{props.logRecord.renderData.message}</span>
            {props.logRecord.renderData.extraFields.length > 0 && (
              <span>
                <span>{', '}</span>
                {props.logRecord.renderData.extraFields.map(({ key, value }, i) => (
                  <span>
                    {i > 0 && <span>{', '}</span>}
                    <span class={levelBoldClass.value}>{key}</span>
                    <span>{': '}</span>
                    <span class="text-orange-300">{value}</span>
                  </span>
                ))}
              </span>
            )}
          </span>
        </div>

        <div class="text-gray-300">
          <span>{'  '}</span>
          <span class="text-gray-500">at</span>
          <span> </span>
          <span>
            {props.logRecord.filename}:{props.logRecord.line_number}
          </span>
        </div>

        {props.logRecord.renderData.spanLines?.map((span, idx) => (
          <div key={idx} class="text-gray-300">
            <span>{'  '}</span>
            <span class="text-gray-500">in</span>
            <span> </span>
            <span class="font-bold text-indigo-300">{span.name}</span>
            {span.args.length > 0 && (
              <span>
                <span> </span>
                <span class="text-gray-500">with</span>
                <span> </span>
                {span.args.map((arg, i) => (
                  <span>
                    {i > 0 && <span>{', '}</span>}
                    <span class="font-bold text-gray-300">{arg.key}</span>
                    <span>: </span>
                    <span class="text-orange-300">{arg.value}</span>
                  </span>
                ))}
              </span>
            )}
          </div>
        ))}
      </div>
    )
  },
})
</script>

<template>
  <n-modal v-model:show="showing" v-if="store.config !== undefined">
    <n-dialog :showIcon="false" @close="showing = false" style="width: 95%">
      <template #header>
        <div class="text-lg font-bold flex items-center gap-2">
          <span v-if="viewMode === 'live'">📡 实时日志</span>
          <span v-else>
            📂 文件日志
            <n-tag class="ml-2" type="primary" size="small">
              {{ currentFileName }}
            </n-tag>
          </span>
        </div>
      </template>

      <div class="mb-2 flex flex-wrap">
        <n-input-group class="flex-1 mr-4">
          <n-input v-model:value="filterText" placeholder="关键词过滤..." clearable />
          <n-select v-model:value="selectedLevel" :options="logLevelOptions" style="width: 120px" />
        </n-input-group>

        <n-button v-if="viewMode === 'file'" class="mr-2" type="primary" secondary @click="exitFileMode">
          返回实时日志
        </n-button>

        <n-button type="primary" @click="openLogFile">打开日志文件</n-button>
      </div>

      <div class="relative h-[calc(100vh-250px)]!">
        <VList
          ref="vListRef"
          class="h-full overflow-hidden bg-gray-950 text-sm"
          :data="filteredLogs"
          @scroll="updateScrollEdgeState"
          #default="{ item }: { item: LogRecord }">
          <LogRecordComponent :key="item.id" :logRecord="item" />
        </VList>

        <div v-show="isAtTop === false" class="absolute top-6 right-6">
          <n-button circle type="primary" class="opacity-30 hover:opacity-100 transition-opacity" @click="jumpToTop">
            <template #icon>
              <n-icon>
                <PhArrowUp />
              </n-icon>
            </template>
          </n-button>
        </div>

        <div v-show="isAtBottom === false" class="absolute bottom-6 right-6">
          <n-button circle type="primary" class="opacity-30 hover:opacity-100 transition-opacity" @click="jumpToBottom">
            <template #icon>
              <n-icon>
                <PhArrowDown />
              </n-icon>
            </template>
          </n-button>
        </div>
      </div>

      <div class="pt-2 flex flex-wrap items-center">
        <n-checkbox v-model:checked="store.config.enableFileLogger">输出文件日志</n-checkbox>
        <n-button class="ml-2" size="small" @click="showLogsDirInFileManager">打开日志目录</n-button>
        <n-tag class="ml-1" size="small" :bordered="false">
          {{ formatedLogsDirSize }}
        </n-tag>

        <n-button
          v-if="viewMode === 'live'"
          ghost
          class="ml-auto"
          size="small"
          type="error"
          @click="clearLiveLogRecords">
          清空实时日志
        </n-button>
      </div>
    </n-dialog>
  </n-modal>
</template>
