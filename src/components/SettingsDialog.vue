<script setup lang="ts">
import { commands } from '../bindings.ts'
import { ref } from 'vue'
import { path } from '@tauri-apps/api'
import { appDataDir } from '@tauri-apps/api/path'
import { useStore } from '../store.ts'

const store = useStore()

const showing = defineModel<boolean>('showing', { required: true })

const customApiDomain = ref<string>(store.config?.customApiDomain ?? '')

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
    <n-dialog :showIcon="false" title="设置" content-style="" @close="showing = false">
      <div class="flex flex-col gap-row-2">
        <n-radio-group v-model:value="store.config.downloadFormat">
          图片下载格式：
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-radio value="Webp">webp</n-radio>
            </template>
            推荐使用，这是拷贝服务器上的原图格式
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-radio value="Jpeg">jpg</n-radio>
            </template>
            拷贝服务器上的原图格式为webp
            <br />
            这个选项会将下载到的webp转为jpg
            <br />
            格式转换过程会导致图片质量损失
          </n-tooltip>
        </n-radio-group>

        <n-radio-group v-model:value="store.config.apiDomainMode">
          API域名：
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

        <n-button class="ml-auto mt-2" size="small" @click="showConfigInFileManager">打开配置目录</n-button>
      </div>
    </n-dialog>
  </n-modal>
</template>
