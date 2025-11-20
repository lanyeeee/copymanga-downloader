import { defineStore } from 'pinia'
import { CurrentTabName, ProgressData } from './types.ts'
import { Comic, Config, GetFavoriteResult, SearchResult, UserProfileRespData } from './bindings.ts'
import { ref } from 'vue'
import { ProgressesPaneTabName } from './panes/ProgressesPane/ProgressesPane.vue'

export const useStore = defineStore('store', () => {
  const config = ref<Config>()
  const userProfile = ref<UserProfileRespData>()
  const pickedComic = ref<Comic>()
  const currentTabName = ref<CurrentTabName>('search')
  const searchResult = ref<SearchResult>()
  const getFavoriteResult = ref<GetFavoriteResult>()
  const progresses = ref<Map<string, ProgressData>>(new Map())
  const progressesPaneTabName = ref<ProgressesPaneTabName>('uncompleted')

  return {
    config,
    userProfile,
    pickedComic,
    currentTabName,
    searchResult,
    getFavoriteResult,
    progresses,
    progressesPaneTabName,
  }
})
