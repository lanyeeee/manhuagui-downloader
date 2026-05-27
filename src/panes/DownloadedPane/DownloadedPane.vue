<script setup lang="ts">
import { Comic, commands } from '../../bindings.ts'
import DownloadedComicCard from './components/DownloadedComicCard.vue'
import { open } from '@tauri-apps/plugin-dialog'
import { computed, ref, watch } from 'vue'
import { useStore } from '../../store.ts'
import { NButton, NIcon, NInput, NInputGroup, NInputGroupLabel, NPagination } from 'naive-ui'
import { PhFolderOpen } from '@phosphor-icons/vue'
import UpdateDownloadedComicsButton from './components/UpdateDownloadedComicsButton.vue'

const store = useStore()

const currentPage = ref<number>(1)

const PAGE_SIZE = 20
const pageCount = computed(() => {
  if (store.downloadedComics.length === 0) {
    return 1
  }

  return Math.ceil(store.downloadedComics.length / PAGE_SIZE)
})
const showingDownloadedComics = computed<Comic[]>(() => {
  const start = (currentPage.value - 1) * PAGE_SIZE
  const end = currentPage.value * PAGE_SIZE
  return store.downloadedComics.slice(start, end)
})

watch(
  () => store.currentTabName,
  async () => {
    if (store.currentTabName !== 'downloaded') {
      return
    }

    store.downloadedComics = await commands.getDownloadedComics()
  },
  { immediate: true },
)

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
