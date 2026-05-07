<script setup lang="ts">
import { Comic, commands, events } from '../../bindings.ts'
import DownloadedComicCard from './components/DownloadedComicCard.vue'
import { open } from '@tauri-apps/plugin-dialog'
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useStore } from '../../store.ts'
import {
  MessageReactive,
  NButton,
  NIcon,
  NInput,
  NInputGroup,
  NInputGroupLabel,
  NPagination,
  useMessage,
} from 'naive-ui'
import { PhFolderOpen } from '@phosphor-icons/vue'
import UpdateDownloadedComicsButton from './components/UpdateDownloadedComicsButton.vue'

interface ProgressData {
  comicTitle: string
  current: number
  totalImgCount: number
  progressMessage: MessageReactive
}

const store = useStore()

const message = useMessage()

const downloadedComics = ref<Comic[]>([])
const currentPage = ref<number>(1)

const PAGE_SIZE = 20
const pageCount = computed(() => {
  if (downloadedComics.value.length === 0) {
    return 1
  }

  return Math.ceil(downloadedComics.value.length / PAGE_SIZE)
})
const showingDownloadedComics = computed<Comic[]>(() => {
  const start = (currentPage.value - 1) * PAGE_SIZE
  const end = currentPage.value * PAGE_SIZE
  return downloadedComics.value.slice(start, end)
})

watch(
  () => store.currentTabName,
  async () => {
    if (store.currentTabName !== 'downloaded') {
      return
    }

    const result = await commands.getDownloadedComics()
    if (result.status === 'error') {
      console.error(result.error)
      return
    }
    downloadedComics.value = result.data
  },
  { immediate: true },
)

const progresses = ref<Map<string, ProgressData>>(new Map())
let unListenExportCbzEvent: () => void | undefined
let unListenExportPdfEvent: () => void | undefined
onMounted(() => {
  events.exportCbzEvent
    .listen(async ({ payload: exportCbzEvent }) => {
      if (exportCbzEvent.event === 'Start') {
        const { uuid, comicTitle, total } = exportCbzEvent.data
        progresses.value.set(uuid, {
          comicTitle,
          current: 0,
          totalImgCount: total,
          progressMessage: message.loading(`${comicTitle} 正在导出cbz(0/${total})`, { duration: 0 }),
        })
      } else if (exportCbzEvent.event === 'Progress') {
        const { uuid, current } = exportCbzEvent.data
        const progressData = progresses.value.get(uuid)
        if (progressData === undefined) {
          return
        }
        progressData.current = current
        progressData.progressMessage.content = `${progressData.comicTitle} 正在导出cbz(${current}/${progressData.totalImgCount})`
      } else if (exportCbzEvent.event === 'End') {
        const { uuid } = exportCbzEvent.data
        const progressData = progresses.value.get(uuid)
        if (progressData === undefined) {
          return
        }
        progressData.progressMessage.type = 'success'
        progressData.progressMessage.content = `${progressData.comicTitle} 导出cbz完成(${progressData.totalImgCount}/${progressData.totalImgCount})`
        setTimeout(() => {
          progressData.progressMessage.destroy()
          progresses.value.delete(uuid)
        }, 3000)
      }
    })
    .then((unListenFn) => {
      unListenExportCbzEvent = unListenFn
    })

  events.exportPdfEvent
    .listen(async ({ payload: exportPdfEvent }) => {
      if (exportPdfEvent.event === 'CreateStart') {
        const { uuid, comicTitle, total } = exportPdfEvent.data
        progresses.value.set(uuid, {
          comicTitle,
          current: 0,
          totalImgCount: total,
          progressMessage: message.loading(`${comicTitle} 正在导出pdf(0/${total})`, { duration: 0 }),
        })
      } else if (exportPdfEvent.event === 'CreateProgress') {
        const { uuid, current } = exportPdfEvent.data
        const progressData = progresses.value.get(uuid)
        if (progressData === undefined) {
          return
        }
        progressData.current = current
        progressData.progressMessage.content = `${progressData.comicTitle} 正在导出pdf(${current}/${progressData.totalImgCount})`
      } else if (exportPdfEvent.event === 'CreateEnd') {
        const { uuid } = exportPdfEvent.data
        const progressData = progresses.value.get(uuid)
        if (progressData === undefined) {
          return
        }
        progressData.progressMessage.type = 'success'
        progressData.progressMessage.content = `${progressData.comicTitle} 导出pdf完成(${progressData.totalImgCount}/${progressData.totalImgCount})`
        setTimeout(() => {
          progressData.progressMessage.destroy()
          progresses.value.delete(uuid)
        }, 3000)
      } else if (exportPdfEvent.event === 'MergeStart') {
        const { uuid, comicTitle, total } = exportPdfEvent.data
        progresses.value.set(uuid, {
          comicTitle,
          current: 0,
          totalImgCount: total,
          progressMessage: message.loading(`${comicTitle} 正在合并pdf(0/${total})`, { duration: 0 }),
        })
      } else if (exportPdfEvent.event === 'MergeProgress') {
        const { uuid, current } = exportPdfEvent.data
        const progressData = progresses.value.get(uuid)
        if (progressData === undefined) {
          return
        }
        progressData.current = current
        progressData.progressMessage.content = `${progressData.comicTitle} 正在合并pdf(${current}/${progressData.totalImgCount})`
      } else if (exportPdfEvent.event === 'MergeEnd') {
        const { uuid } = exportPdfEvent.data
        const progressData = progresses.value.get(uuid)
        if (progressData === undefined) {
          return
        }
        progressData.progressMessage.type = 'success'
        progressData.progressMessage.content = `${progressData.comicTitle} 合并pdf完成(${progressData.totalImgCount}/${progressData.totalImgCount})`
        setTimeout(() => {
          progressData.progressMessage.destroy()
          progresses.value.delete(uuid)
        }, 3000)
      }
    })
    .then((unListenFn) => {
      unListenExportPdfEvent = unListenFn
    })
})

onUnmounted(() => {
  unListenExportCbzEvent?.()
  unListenExportPdfEvent?.()
})

async function selectExportDir() {
  if (store.config === undefined) {
    return
  }

  const selectedDirPath = await open({ directory: true })
  if (selectedDirPath === null) {
    return
  }

  store.config.exportDir = selectedDirPath
}

async function showExportDirInFileManager() {
  if (store.config === undefined) {
    return
  }

  const result = await commands.showPathInFileManager(store.config.exportDir)
  if (result.status === 'error') {
    console.error(result.error)
  }
}
</script>

<template>
  <div class="h-full flex flex-col overflow-auto">
    <div class="flex gap-1 box-border px-2 pt-2">
      <n-input-group class="whitespace-nowrap">
        <n-input-group-label size="small">导出目录</n-input-group-label>
        <n-input size="small" readonly :value="store.config?.exportDir" @click="selectExportDir" />
        <n-button class="w-10" size="small" @click="showExportDirInFileManager">
          <template #icon>
            <n-icon size="20">
              <PhFolderOpen />
            </n-icon>
          </template>
        </n-button>
      </n-input-group>
      <UpdateDownloadedComicsButton />
    </div>

    <div class="h-full flex flex-col gap-row-1 overflow-auto">
      <div class="h-full flex flex-col gap-row-2 overflow-auto p-2">
        <downloaded-comic-card v-for="comic in showingDownloadedComics" :key="comic.id" :comic="comic" />
      </div>

      <n-pagination class="p-2 mt-auto" v-model:page="currentPage" :page-count="pageCount" />
    </div>
  </div>
</template>
