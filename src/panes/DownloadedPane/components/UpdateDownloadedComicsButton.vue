<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { MessageReactive, useMessage } from 'naive-ui'
import { commands, events, UpdateDownloadedComicsEvent } from '../../../bindings.ts'

const message = useMessage()

type ProgressData = Extract<UpdateDownloadedComicsEvent, { event: 'CreateDownloadTasksStart' }>['data'] & {
  progressMessage: MessageReactive
}

const progresses = ref<Map<string, ProgressData>>(new Map())
let updateMessage: MessageReactive | undefined

onMounted(async () => {
  await events.updateDownloadedComicsEvent.listen(async ({ payload: updateEvent }) => {
    if (updateEvent.event === 'GetComicStart') {
      updateMessage = message.loading(`正在获取已下载漫画的最新数据`, { duration: 0 })
    } else if (updateEvent.event === 'GetComicProgress' && updateMessage !== undefined) {
      const { current, total } = updateEvent.data
      updateMessage.content = `正在获取已下载漫画的最新数据(${current}/${total})`
    } else if (updateEvent.event === 'CreateDownloadTasksStart') {
      const { comicPathWord, comicTitle, current, total } = updateEvent.data
      progresses.value.set(comicPathWord, {
        comicPathWord,
        comicTitle,
        current,
        total,
        progressMessage: message.loading(
          () => {
            const progressData = progresses.value.get(comicPathWord)
            if (progressData === undefined) return ''
            return `${progressData.comicTitle} 正在创建下载任务(${progressData.current}/${progressData.total})`
          },
          { duration: 0 },
        ),
      })
    } else if (updateEvent.event === 'CreateDownloadTaskProgress') {
      const { comicPathWord, current } = updateEvent.data
      const progressData = progresses.value.get(comicPathWord)
      if (progressData) {
        progressData.current = current
      }
    } else if (updateEvent.event === 'CreateDownloadTasksEnd' && updateMessage !== undefined) {
      const { comicPathWord } = updateEvent.data
      const progressData = progresses.value.get(comicPathWord)
      if (progressData) {
        progressData.progressMessage.type = 'success'
        progressData.progressMessage.content = `${progressData.comicTitle} 创建下载任务完成(${progressData.current}/${progressData.total})`
        setTimeout(() => {
          progressData.progressMessage.destroy()
          progresses.value.delete(comicPathWord)
        }, 3000)
      }
    } else if (updateEvent.event === 'GetComicEnd' && updateMessage !== undefined) {
      updateMessage.type = 'success'
      updateMessage.content = '已获取所有已下载漫画的最新数据，并为需要更新的章节创建了下载任务'
      setTimeout(() => {
        updateMessage?.destroy()
        updateMessage = undefined
      }, 5000)
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
