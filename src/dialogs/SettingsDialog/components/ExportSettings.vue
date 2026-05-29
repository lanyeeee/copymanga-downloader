<script setup lang="ts">
import { ref } from 'vue'
import { useStore } from '../../../store.ts'
import {
  NInput,
  NInputGroup,
  NInputGroupLabel,
  NInputNumber,
  NTooltip,
  NCheckbox,
  NSelect,
  NConfigProvider,
  NPopover,
} from 'naive-ui'

const store = useStore()

const exportDirFmt = ref<string>(store.config?.exportDirFmt ?? '')
const mergePdfFmt = ref<string>(store.config?.mergePdfFmt ?? '')

const exportSkipModeOptions = [
  { label: '不跳过，每次都重新导出', value: 'None' },
  { label: '跳过已存在的文件', value: 'SkipExisting' },
  { label: '跳过曾导出过的章节', value: 'SkipExported' },
]
</script>

<template>
  <div v-if="store.config !== undefined" class="flex flex-col">
    <div class="flex gap-1 items-center">
      <n-input-group class="w-70">
        <n-input-group-label size="small">创建pdf并发数</n-input-group-label>
        <n-input-number
          class="w-full"
          v-model:value="store.config.createPdfConcurrency"
          size="small"
          :min="1"
          :parse="(x: string) => Number(x)" />
      </n-input-group>
      <n-tooltip placement="top" trigger="hover">
        <div>
          <span>在</span>
          <span class="rounded bg-gray-500 px-1">章节详情</span>
          <span>里手动勾选导出的PDF一律不会自动合并</span>
        </div>
        <div>
          <span>只有在</span>
          <span class="rounded bg-gray-500 px-1">本地库存</span>
          <span>里直接导出整部作品为PDF</span>
        </div>
        <div>
          <span>且导出策略不为</span>
          <span class="rounded bg-gray-500 px-1">跳过曾导出过的章节</span>
          <span>时才会触发自动合并</span>
        </div>
        <template #trigger>
          <n-checkbox class="ml-4 w-fit" v-model:checked="store.config.enableMergePdf">创建完成后自动合并</n-checkbox>
        </template>
      </n-tooltip>
    </div>
    <n-tooltip placement="top" trigger="hover">
      <div>
        <span>只影响</span>
        <span class="rounded bg-gray-500 px-1">本地库存</span>
        <span>里直接导出整部作品时的行为</span>
      </div>
      <div>
        <span>在</span>
        <span class="rounded bg-gray-500 px-1">章节详情</span>
        <span>里手动勾选导出时一律以</span>
        <span class="rounded bg-gray-500 px-1">不跳过，每次都重新导出</span>
        <span>处理</span>
      </div>
      <template #trigger>
        <n-input-group class="mt-1 w-fit">
          <n-input-group-label size="small">导出策略</n-input-group-label>
          <n-select
            v-model:value="store.config.exportSkipMode"
            :options="exportSkipModeOptions"
            size="small"
            class="w-50" />
        </n-input-group>
      </template>
    </n-tooltip>

    <span class="font-bold mt-2">合并pdf目录格式</span>
    <n-config-provider
      :theme-overrides="{ Scrollbar: { color: 'rgba(255, 255, 255, 0.25)', colorHover: 'rgba(255, 255, 255, 0.3)' } }">
      <n-tooltip placement="top" trigger="hover" class="max-h-45vh" :scrollable="true">
        <div>
          可以用斜杠
          <span class="rounded bg-gray-500 px-1 select-all text-white">/</span>
          来分隔目录层级，最后一层会作为合并后的PDF文件名
        </div>
        <div class="mt-1">
          整个模板里至少要包含一个
          <span class="rounded bg-gray-500 px-1 select-all">分组字段</span>
          ，这样不同组的合并结果才不会互相覆盖
        </div>
        <div class="font-semibold mt-2">
          <span>可用字段：</span>
        </div>
        <div>
          <span class="rounded bg-gray-500 px-1 select-all">comic_uuid</span>
          <span class="ml-2">漫画ID</span>
        </div>
        <div>
          <span class="rounded bg-gray-500 px-1 select-all">comic_path_word</span>
          <span class="ml-2">漫画字母路径</span>
        </div>
        <div>
          <span class="rounded bg-gray-500 px-1 select-all">comic_title</span>
          <span class="ml-2">漫画标题</span>
        </div>
        <div>
          <span class="rounded bg-gray-500 px-1 select-all">author</span>
          <span class="ml-2">作者</span>
        </div>
        <div>
          <span class="rounded bg-gray-500 px-1 select-all">group_path_word</span>
          <span class="ml-2">分组字母路径</span>
        </div>
        <div>
          <span class="rounded bg-gray-500 px-1 select-all">group_title</span>
          <span class="ml-2">分组标题（默認、单行本...）</span>
        </div>
        <div class="font-semibold mt-2">例如格式</div>
        <div class="bg-gray-200 rounded-md p-1 text-black w-fit">{comic_title}/pdf/{group_title}</div>
        <div class="font-semibold">
          <span>导出</span>
          <span class="text-blue mx-1">電鋸人</span>
          <span>并触发自动合并时，会在</span>
        </div>
        <div class="flex gap-1">
          <span class="bg-gray-200 rounded-md px-1 w-fit text-black">電鋸人</span>
          <span class="rounded bg-gray-500 px-1 select-all text-white">/</span>
          <span class="bg-gray-200 rounded-md px-1 w-fit text-black">pdf</span>
        </div>
        <div class="font-semibold">
          下生成每个分组自己的合并文件，例如
          <span class="bg-gray-200 rounded-md px-1 w-fit text-black font-normal ml-1">默認.pdf</span>
        </div>
        <template #trigger>
          <n-input
            v-model:value="mergePdfFmt"
            size="small"
            @blur="store.config.mergePdfFmt = mergePdfFmt"
            @keydown.enter="store.config.mergePdfFmt = mergePdfFmt" />
        </template>
      </n-tooltip>
    </n-config-provider>

    <span class="font-bold mt-2">导出目录格式</span>
    <n-config-provider
      :theme-overrides="{ Scrollbar: { color: 'rgba(255, 255, 255, 0.25)', colorHover: 'rgba(255, 255, 255, 0.3)' } }">
      <n-tooltip placement="top" trigger="hover" :width="600" class="max-h-55vh" :scrollable="true">
        <div>
          可以用斜杠
          <span class="rounded bg-gray-500 px-1 select-all text-white">/</span>
          来分隔目录层级，最后一层会作为导出文件名
        </div>
        <div class="mt-1">
          <span class="rounded bg-gray-500 px-1 select-all">章节字段</span>
          只能出现在最后一层，这是为了保证同一组里的不同章节不会导出到同一路径
        </div>
        <div class="mt-1">
          最后一层必须至少包含一个
          <span class="rounded bg-gray-500 px-1 select-all">章节字段</span>
        </div>
        <div class="grid grid-cols-2 gap-x-2">
          <div class="flex flex-col">
            <div class="font-semibold mt-2">通用字段：</div>
            <div>
              <span class="rounded bg-gray-500 px-1 select-all">comic_uuid</span>
              <span class="ml-2">漫画ID</span>
            </div>
            <div>
              <span class="rounded bg-gray-500 px-1 select-all">comic_path_word</span>
              <span class="ml-2">漫画字母路径</span>
            </div>
            <div>
              <span class="rounded bg-gray-500 px-1 select-all">comic_title</span>
              <span class="ml-2">漫画标题</span>
            </div>
            <div>
              <span class="rounded bg-gray-500 px-1 select-all">author</span>
              <span class="ml-2">作者</span>
            </div>
            <div>
              <span class="rounded bg-gray-500 px-1 select-all">export_format</span>
              <span class="ml-2">导出格式(pdf/cbz)</span>
            </div>
          </div>

          <div class="flex flex-col">
            <div class="font-semibold mt-2">章节字段：</div>
            <div>
              <span class="rounded bg-gray-500 px-1 select-all">chapter_uuid</span>
              <span class="ml-2">章节ID</span>
            </div>
            <div>
              <span class="rounded bg-gray-500 px-1 select-all">chapter_title</span>
              <span class="ml-2">章节标题</span>
            </div>
            <div>
              <span class="rounded bg-gray-500 px-1 select-all">order</span>
              <span class="ml-2">章节在分组中的顺序，一些特殊章节会有小数点，</span>
              <n-popover placement="top" trigger="hover">
                <template #trigger><span class="text-blue">支持补齐</span></template>
                <div class="text-xs">
                  <span>示例：</span>
                  <span class="rounded bg-gray-300 px-1 select-all font-mono">{order:0>3}</span>
                  <span>表示用0补齐3位，</span>
                  <span class="mr-2">例如 13 &rarr; 013</span>
                  <span>13.1 &rarr; 013.1</span>
                </div>
              </n-popover>
            </div>

            <div class="font-semibold mt-2">分组字段：</div>
            <div>
              <span class="rounded bg-gray-500 px-1 select-all">group_path_word</span>
              <span class="ml-2">分组字母路径</span>
            </div>
            <div>
              <span class="rounded bg-gray-500 px-1 select-all">group_title</span>
              <span class="ml-2">分组标题（默認、单行本...）</span>
            </div>
          </div>
        </div>
        <div class="font-semibold mt-2">例如格式</div>
        <div class="bg-gray-200 rounded-md p-1 text-black w-fit">
          {comic_title}/{export_format}/{group_title}/{order:0>3} {chapter_title}
        </div>
        <div class="font-semibold">
          <span>导出</span>
          <span class="text-blue mx-1">電鋸人 - 默認 - 第13话</span>
          <span>为pdf时，会创建</span>
        </div>
        <div class="flex gap-1">
          <span class="bg-gray-200 rounded-md px-1 w-fit text-black">電鋸人</span>
          <span class="rounded bg-gray-500 px-1 select-all text-white">/</span>
          <span class="bg-gray-200 rounded-md px-1 w-fit text-black">pdf</span>
          <span class="rounded bg-gray-500 px-1 select-all text-white">/</span>
          <span class="bg-gray-200 rounded-md px-1 w-fit text-black">默認</span>
        </div>
        <div class="font-semibold">
          三层目录，并在最内层生成
          <span class="bg-gray-200 rounded-md px-1 w-fit text-black font-normal ml-1">013 第13话.pdf</span>
        </div>
        <template #trigger>
          <n-input
            v-model:value="exportDirFmt"
            size="small"
            @blur="store.config.exportDirFmt = exportDirFmt"
            @keydown.enter="store.config.exportDirFmt = exportDirFmt" />
        </template>
      </n-tooltip>
    </n-config-provider>
  </div>
</template>
