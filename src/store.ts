import { defineStore } from 'pinia'
import { Comic, Config, GetFavoriteResult, SearchResult, UserProfile } from './bindings.ts'
import { CurrentTabName, ProgressData } from './types.ts'
import { ref } from 'vue'
import { ProgressesPaneTabName } from './panes/ProgressesPane/ProgressesPane.vue'

export const useStore = defineStore('store', () => {
  const config = ref<Config>()
  const userProfile = ref<UserProfile>()
  const pickedComic = ref<Comic>()
  const currentTabName = ref<CurrentTabName>('search')
  const searchResult = ref<SearchResult>()
  const getFavoriteResult = ref<GetFavoriteResult>()
  const progresses = ref<Map<number, ProgressData>>(new Map())
  const progressesPaneTabName = ref<ProgressesPaneTabName>('uncompleted')
  const downloadedComics = ref<Comic[]>([])

  return {
    config,
    userProfile,
    pickedComic,
    currentTabName,
    searchResult,
    getFavoriteResult,
    progresses,
    progressesPaneTabName,
    downloadedComics,
  }
})
