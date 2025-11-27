<script setup lang="ts">
import { commands } from '../bindings.ts'
import { ref } from 'vue'
import { path } from '@tauri-apps/api'
import { appDataDir } from '@tauri-apps/api/path'
import { useStore } from '../store.ts'
import { useMessage } from 'naive-ui'

const store = useStore()

const message = useMessage()

const showing = defineModel<boolean>('showing', { required: true })

const customApiDomain = ref<string>(store.config?.customApiDomain ?? '')
const comicDirFmt = ref<string>(store.config?.comicDirFmt ?? '')
const chapterDirFmt = ref<string>(store.config?.chapterDirFmt ?? '')

async function showConfigInFileManager() {
  const configName = 'config.json'
  const configPath = await path.join(await appDataDir(), configName)
  const result = await commands.showPathInFileManager(configPath)
  if (result.status === 'error') {
    console.error(result.error)
  }
}
</script>

<template>
  <n-modal v-if="store.config !== undefined" v-model:show="showing">
    <n-dialog class="w-140!" :showIcon="false" title="设置" content-style="" @close="showing = false">
      <div class="flex flex-col">
        <span class="font-bold">图片下载格式</span>
        <n-radio-group v-model:value="store.config.downloadFormat">
          <n-tooltip placement="top" trigger="hover">
            <div>推荐使用，这是拷贝服务器上的原图格式</div>
            <div class="text-blue">不过导出pdf较慢</div>
            <template #trigger>
              <n-radio value="Webp">webp</n-radio>
            </template>
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <div>拷贝服务器上的原图格式为webp</div>
            <div>这个选项会将下载到的webp转为jpg</div>
            <div>格式转换过程会导致图片质量损失</div>
            <div class="text-blue">好处是导出pdf很快</div>
            <template #trigger>
              <n-radio value="Jpeg">jpg</n-radio>
            </template>
          </n-tooltip>
        </n-radio-group>

        <span class="font-bold mt-2">API域名</span>
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

        <span class="font-bold mt-2">漫画目录格式</span>
        <n-tooltip placement="top" trigger="hover">
          <div>
            可以用斜杠
            <span class="rounded bg-gray-500 px-1 select-all text-white">/</span>
            来分隔目录层级
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
          <div class="font-semibold mt-2">例如格式</div>
          <div class="bg-gray-200 rounded-md p-1 text-black w-fit">{author}/{comic_title}</div>
          <div class="font-semibold">
            <span>下载</span>
            <span class="text-blue mx-1">電鋸人</span>
            <span>的任何一个章节会创建</span>
          </div>
          <div class="flex gap-1">
            <span class="bg-gray-200 rounded-md px-1 w-fit text-black">藤本タツキ</span>
            <span class="rounded bg-gray-500 px-1 select-all text-white">/</span>
            <span class="bg-gray-200 rounded-md px-1 w-fit text-black">電鋸人</span>
          </div>
          <div class="font-semibold">
            两层文件夹，漫画元数据保存在最内层的文件夹
            <span class="bg-gray-200 rounded-md px-1 w-fit text-black font-normal">電鋸人</span>
            里
          </div>
          <template #trigger>
            <n-input
              v-model:value="comicDirFmt"
              size="small"
              @blur="store.config.comicDirFmt = comicDirFmt"
              @keydown.enter="store.config.comicDirFmt = comicDirFmt" />
          </template>
        </n-tooltip>

        <span class="font-bold mt-2">章节目录格式</span>
        <n-tooltip placement="top" trigger="hover">
          <div>
            可以用斜杠
            <span class="rounded bg-gray-500 px-1 select-all text-white">/</span>
            来分隔目录层级
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
            <span class="ml-2">章节在分组中的序号，一些特殊章节会有小数点</span>
          </div>
          <div class="font-semibold mt-2">例如格式</div>
          <div class="bg-gray-200 rounded-md p-1 text-black w-fit">{group_title}/{order} {chapter_title}</div>
          <div class="font-semibold">
            <span>下载</span>
            <span class="text-blue mx-1">電鋸人 - 默認 - 第13话</span>
            <span>会在漫画目录下再创建</span>
          </div>
          <div class="flex gap-1">
            <span class="bg-gray-200 rounded-md px-1 w-fit text-black">默認</span>
            <span class="rounded bg-gray-500 px-1 select-all text-white">/</span>
            <span class="bg-gray-200 rounded-md px-1 w-fit text-black">13 第13话</span>
          </div>
          <div class="font-semibold">
            两层文件夹，章节元数据保存在最内层的文件夹
            <span class="bg-gray-200 rounded-md px-1 w-fit text-black font-normal">13 第13话</span>
            里
          </div>
          <template #trigger>
            <n-input
              v-model:value="chapterDirFmt"
              size="small"
              @blur="store.config.chapterDirFmt = chapterDirFmt"
              @keydown.enter="store.config.chapterDirFmt = chapterDirFmt" />
          </template>
        </n-tooltip>

        <n-button class="ml-auto mt-2" size="small" @click="showConfigInFileManager">打开配置目录</n-button>
      </div>
    </n-dialog>
  </n-modal>
</template>
