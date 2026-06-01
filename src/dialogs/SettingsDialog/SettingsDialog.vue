<script setup lang="ts">
import { commands } from '../../bindings.ts'
import { path } from '@tauri-apps/api'
import { appDataDir } from '@tauri-apps/api/path'
import { useStore } from '../../store.ts'
import { NButton, NDialog, NModal, NTabs, NTabPane } from 'naive-ui'
import DownloadSettings from './components/DownloadSettings.vue'
import NetworkSettings from './components/NetworkSettings.vue'
import ExportSettings from './components/ExportSettings.vue'
import { ref } from 'vue'

const store = useStore()

const showing = defineModel<boolean>('showing', { required: true })

const currentTabName = ref<string>('download_settings')

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
    <n-dialog class="w-140!" :showIcon="false" content-style="" @close="showing = false">
      <div class="flex flex-col gap-row-2">
        <n-tabs class="h-full" v-model:value="currentTabName" type="line" size="small" animated>
          <n-tab-pane name="download_settings" tab="下载相关">
            <DownloadSettings />
          </n-tab-pane>
          <n-tab-pane name="network_settings" tab="网络相关">
            <NetworkSettings />
          </n-tab-pane>
          <n-tab-pane name="export_settings" tab="导出相关">
            <ExportSettings />
          </n-tab-pane>
        </n-tabs>

        <n-button class="ml-auto mt-2" size="small" @click="showConfigInFileManager">打开配置目录</n-button>
      </div>
    </n-dialog>
  </n-modal>
</template>
