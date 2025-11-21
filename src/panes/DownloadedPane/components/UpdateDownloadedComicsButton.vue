<script setup lang="ts">
import { onMounted } from 'vue'
import { MessageReactive, useMessage, useNotification } from 'naive-ui'
import { commands, events } from '../../../bindings.ts'

const message = useMessage()
const notification = useNotification()

let updateMessage: MessageReactive | undefined

onMounted(async () => {
  await events.updateDownloadedComicsEvent.listen(async ({ payload: updateEvent }) => {
    if (updateEvent.event === 'GettingComics') {
      const { total } = updateEvent.data
      updateMessage = message.loading(`正在获取已下载漫画的最新数据(0/${total})`, { duration: 0 })
    } else if (updateEvent.event === 'GetComicError' && updateMessage !== undefined) {
      const { comicTitle, errMsg } = updateEvent.data
      notification.warning({
        title: `获取漫画 ${comicTitle} 的数据失败`,
        description: errMsg,
        duration: 0,
      })
    } else if (updateEvent.event === 'ComicGot' && updateMessage !== undefined) {
      const { current, total } = updateEvent.data
      updateMessage.content = `正在获取已下载漫画的最新数据(${current}/${total})`
    } else if (updateEvent.event === 'DownloadTaskCreated' && updateMessage !== undefined) {
      updateMessage.type = 'success'
      updateMessage.content = '已为需要更新的章节创建下载任务'
      setTimeout(() => {
        updateMessage?.destroy()
        updateMessage = undefined
      }, 3000)
    }
  })
})

// 更新已下载漫画
async function updateDownloadedComics() {
  const result = await commands.updateDownloadedComics()
  if (result.status === 'error') {
    setTimeout(() => {
      updateMessage?.destroy()
      updateMessage = undefined
    }, 3000)
    console.error(result.error)
  }
}
</script>

<template>
  <n-button size="small" @click="updateDownloadedComics">更新库存</n-button>
</template>
