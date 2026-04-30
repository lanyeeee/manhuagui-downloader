<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import { PhFolderOpen } from '@phosphor-icons/vue'
import { commands, events } from '../../bindings.ts'
import UncompletedProgresses from './components/UncompletedProgresses.vue'
import CompletedProgresses from './components/CompletedProgresses.vue'
import { useStore } from '../../store.ts'
import { ProgressData } from '../../types.ts'

const store = useStore()

const downloadSpeed = ref<string>('')

let unListenDownloadSpeedEvent: () => void | undefined
let unListenDownloadSleepingEvent: () => void | undefined
let unListenDownloadTaskEvent: () => void | undefined

onMounted(() => {
  events.downloadSpeedEvent
    .listen(async ({ payload: { speed } }) => {
      downloadSpeed.value = speed
    })
    .then((unListenFn) => {
      unListenDownloadSpeedEvent = unListenFn
    })

  events.downloadSleepingEvent
    .listen(async ({ payload: { chapterId, remainingSec } }) => {
      const progressData = store.progresses.get(chapterId)
      if (progressData !== undefined) {
        progressData.indicator = `将在${remainingSec}秒后继续下载`
      }
    })
    .then((unListenFn) => {
      unListenDownloadSleepingEvent = unListenFn
    })

  events.downloadTaskEvent
    .listen(async ({ payload: { event, data } }) => {
      if (event === 'Create') {
        const { chapterInfo, downloadedImgCount, totalImgCount } = data

        store.progresses.set(chapterInfo.chapterId, {
          ...data,
          percentage: 0,
          indicator: `排队中 ${downloadedImgCount}/${totalImgCount}`,
        })
      } else if (event === 'Update') {
        const { state, chapterId, downloadedImgCount, totalImgCount } = data

        const progressData = store.progresses.get(chapterId)
        if (progressData === undefined) {
          return
        }

        progressData.state = state
        progressData.downloadedImgCount = downloadedImgCount
        progressData.totalImgCount = totalImgCount
        progressData.percentage = (downloadedImgCount / totalImgCount) * 100

        if (state === 'Completed') {
          progressData.chapterInfo.isDownloaded = true
          await syncPickedComic()
          await syncComicInSearch(progressData)
          await syncComicInFavorite(progressData)
        }

        let indicator = ''
        if (state === 'Pending') {
          indicator = '排队中'
        } else if (state === 'Downloading') {
          indicator = '下载中'
        } else if (state === 'Paused') {
          indicator = '已暂停'
        } else if (state === 'Cancelled') {
          indicator = '已取消'
        } else if (state === 'Completed') {
          indicator = '下载完成'
        } else if (state === 'Failed') {
          indicator = '下载失败'
        }

        if (totalImgCount !== 0) {
          indicator += ` ${downloadedImgCount}/${totalImgCount}`
        }

        progressData.indicator = indicator
      }
    })
    .then((unListenFn) => {
      unListenDownloadTaskEvent = unListenFn
    })
})

onUnmounted(() => {
  unListenDownloadSpeedEvent?.()
  unListenDownloadSleepingEvent?.()
  unListenDownloadTaskEvent?.()
})

async function syncPickedComic() {
  if (store.pickedComic === undefined) {
    return
  }

  const syncedComic = await commands.getSyncedComic(store.pickedComic)

  Object.assign(store.pickedComic, { ...syncedComic })
}

async function syncComicInSearch(progressData: ProgressData) {
  if (store.searchResult === undefined) {
    return
  }

  const comic = store.searchResult.comics.find((comic) => comic.id === progressData.comic.id)
  if (comic === undefined) {
    return
  }

  const syncedComic = await commands.getSyncedComicInSearch(comic)

  Object.assign(comic, { ...syncedComic })
}

async function syncComicInFavorite(progressData: ProgressData) {
  if (store.getFavoriteResult === undefined) {
    return
  }

  const comic = store.getFavoriteResult.comics.find((comic) => comic.id === progressData.comic.id)
  if (comic === undefined) {
    return
  }

  const syncedComic = await commands.getSyncedComicInFavorite(comic)

  Object.assign(comic, { ...syncedComic })
}

async function selectDownloadDir() {
  if (store.config === undefined) {
    return
  }

  const selectedDirPath = await open({ directory: true })
  if (selectedDirPath === null) {
    return
  }
  store.config.downloadDir = selectedDirPath
}

async function showDownloadDirInFileManager() {
  if (store.config === undefined) {
    return
  }

  const result = await commands.showPathInFileManager(store.config.downloadDir)
  if (result.status === 'error') {
    console.error(result.error)
  }
}
</script>

<template>
  <div v-if="store.config !== undefined" class="flex flex-col h-full overflow-auto">
    <div class="box-border px-2 pt-2 whitespace-nowrap">
      <n-input-group>
        <n-input-group-label size="small">下载目录</n-input-group-label>
        <n-input :value="store.config.downloadDir" size="small" readonly @click="selectDownloadDir" />
        <n-button class="w-10" size="small" @click="showDownloadDirInFileManager">
          <template #icon>
            <n-icon size="20">
              <PhFolderOpen />
            </n-icon>
          </template>
        </n-button>
      </n-input-group>
    </div>

    <n-tabs class="overflow-auto h-full mt-2" type="line" size="small" animated>
      <n-tab-pane class="h-full p-0! overflow-auto" name="uncompleted" tab="未完成">
        <uncompleted-progresses />
      </n-tab-pane>

      <n-tab-pane class="h-full p-0! overflow-auto" name="completed" tab="已完成">
        <completed-progresses />
      </n-tab-pane>

      <template #suffix>
        <span class="whitespace-nowrap text-ellipsis overflow-hidden">{{ downloadSpeed }}</span>
      </template>
    </n-tabs>
  </div>
</template>

<style scoped>
:deep(.n-tabs-tab) {
  @apply important-py-0.75;
}
</style>
