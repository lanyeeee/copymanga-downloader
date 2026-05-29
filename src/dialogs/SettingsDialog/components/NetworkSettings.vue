<script setup lang="ts">
import { ref } from 'vue'
import { useStore } from '../../../store.ts'
import { NInput, NInputGroup, NInputGroupLabel, NInputNumber, NRadio, NRadioGroup, useMessage } from 'naive-ui'

const store = useStore()

const message = useMessage()

const customApiDomain = ref<string>(store.config?.customApiDomain ?? '')
</script>

<template>
  <div v-if="store.config !== undefined" class="flex flex-col">
    <span class="font-bold">API域名</span>
    <n-radio-group v-model:value="store.config.apiDomainMode">
      <n-radio value="Default">默认</n-radio>
      <n-radio value="Custom">自定义</n-radio>
    </n-radio-group>
    <n-input-group v-if="store.config.apiDomainMode === 'Custom'">
      <n-input-group-label size="small">自定义API域名</n-input-group-label>
      <n-input
        v-model:value="customApiDomain"
        size="small"
        placeholder=""
        @blur="store.config.customApiDomain = customApiDomain"
        @keydown.enter="store.config.customApiDomain = customApiDomain" />
    </n-input-group>

    <span class="mr-2 font-bold mt-2">下载速度</span>
    <div class="flex flex-col gap-1">
      <div class="flex gap-1">
        <n-input-group class="w-35%">
          <n-input-group-label size="small">章节并发数</n-input-group-label>
          <n-input-number
            class="w-full"
            v-model:value="store.config.chapterConcurrency"
            size="small"
            @update:value="message.warning('对章节并发数的修改需要重启才能生效')"
            :min="1"
            :parse="(x: string) => Number(x)" />
        </n-input-group>
        <n-input-group class="w-65%">
          <n-input-group-label size="small">每个章节下载完成后休息</n-input-group-label>
          <n-input-number
            class="w-full"
            v-model:value="store.config.chapterDownloadIntervalSec"
            size="small"
            :min="0"
            :parse="(x: string) => Number(x)" />
          <n-input-group-label size="small">秒</n-input-group-label>
        </n-input-group>
      </div>
      <div class="flex gap-1">
        <n-input-group class="w-35%">
          <n-input-group-label size="small">图片并发数</n-input-group-label>
          <n-input-number
            class="w-full"
            v-model:value="store.config.imgConcurrency"
            size="small"
            @update-value="message.warning('对图片并发数的修改需要重启才能生效')"
            :min="1"
            :parse="(x: string) => Number(x)" />
        </n-input-group>
        <n-input-group class="w-65%">
          <n-input-group-label size="small">每张图片下载完成后休息</n-input-group-label>
          <n-input-number
            class="w-full"
            v-model:value="store.config.imgDownloadIntervalSec"
            size="small"
            :min="0"
            :parse="(x: string) => Number(x)" />
          <n-input-group-label size="small">秒</n-input-group-label>
        </n-input-group>
      </div>
      <n-input-group>
        <n-input-group-label size="small">更新库存时，每处理完一个已下载的漫画后休息</n-input-group-label>
        <n-input-number
          class="w-full"
          v-model:value="store.config.updateDownloadedComicsIntervalSec"
          size="small"
          :min="0"
          :parse="(x: string) => Number(x)" />
        <n-input-group-label size="small">秒</n-input-group-label>
      </n-input-group>
    </div>
  </div>
</template>
