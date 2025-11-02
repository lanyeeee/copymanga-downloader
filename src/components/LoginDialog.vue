<script setup lang="ts">
import { ref } from 'vue'
import { commands } from '../bindings.ts'
import { useMessage, useNotification } from 'naive-ui'
import FloatLabelInput from './FloatLabelInput.vue'
import { useStore } from '../store.ts'

const store = useStore()

const message = useMessage()
const notification = useNotification()

const showing = defineModel<boolean>('showing', { required: true })

const username = ref<string>('')
const password = ref<string>('')

async function login() {
  if (store.config === undefined) {
    return
  }
  if (username.value === '') {
    message.error('请输入用户名')
    return
  }
  if (password.value === '') {
    message.error('请输入密码')
    return
  }
  const result = await commands.login(username.value, password.value)
  if (result.status === 'error') {
    notification.error({ title: '登录失败', description: result.error })
    return
  }
  message.success('登录成功')
  store.config.token = result.data.token
  showing.value = false
}
</script>

<template>
  <n-modal v-model:show="showing">
    <n-dialog
      class="flex flex-col"
      :showIcon="false"
      title="账号登录"
      positive-text="登录"
      @positive-click="login"
      @close="showing = false"
      @keydown.enter="login">
      <div class="flex flex-col gap-2">
        <FloatLabelInput label="用户名" v-model:value="username" />
        <FloatLabelInput label="密码" v-model:value="password" type="password" />
      </div>
    </n-dialog>
  </n-modal>
</template>
