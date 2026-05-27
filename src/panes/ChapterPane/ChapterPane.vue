<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { ChapterInfo, commands } from '../../bindings.ts'
import { useStore } from '../../store.ts'
import { PhFolderOpen } from '@phosphor-icons/vue'
import IconButton from '../../components/IconButton.vue'
import ChapterDownloadPanel from './components/ChapterDownloadPanel.vue'
import ChapterExportPanel from './components/ChapterExportPanel.vue'
import { NEmpty } from 'naive-ui'

export type ChapterPaneMode = 'download' | 'export'

const store = useStore()

const chapterPaneMode = ref<ChapterPaneMode>('download')
const sortedGroups = computed<[string, ChapterInfo[]][]>(() => {
  if (store.pickedComic === undefined) {
    return []
  }

  return Object.entries(store.pickedComic.groups).sort((a, b) => b[1].length - a[1].length)
})
const firstGroupName = computed(() => sortedGroups.value[0]?.[0] ?? '')
const currentGroupName = ref<string>(firstGroupName.value)

watch(
  () => store.pickedComic,
  () => {
    currentGroupName.value = firstGroupName.value
    chapterPaneMode.value = 'download'
  },
)

async function reloadPickedComic() {
  if (store.pickedComic === undefined) {
    return
  }

  const result = await commands.getComic(store.pickedComic.id)
  if (result.status === 'error') {
    console.error(result.error)
    return
  }

  store.pickedComic = result.data
}

async function showComicDownloadDirInFileManager() {
  if (store.pickedComic === undefined) {
    return
  }

  const comicDownloadDir = store.pickedComic.comicDownloadDir
  if (comicDownloadDir === undefined || comicDownloadDir === null) {
    console.error('comicDownloadDir的值为undefined或null')
    return
  }

  const result = await commands.showPathInFileManager(comicDownloadDir)
  if (result.status === 'error') {
    console.error(result.error)
  }
}
</script>

<template>
  <div class="h-full flex flex-col box-border">
    <n-empty v-if="store.pickedComic === undefined" description="请先选择漫画(漫画搜索、漫画收藏、本地库存)" />
    <template v-else>
      <ChapterDownloadPanel
        v-if="chapterPaneMode === 'download'"
        v-model:chapterPaneMode="chapterPaneMode"
        v-model:currentGroupName="currentGroupName"
        :sortedGroups="sortedGroups"
        :reload="reloadPickedComic" />
      <ChapterExportPanel
        v-else
        v-model:chapterPaneMode="chapterPaneMode"
        v-model:currentGroupName="currentGroupName"
        :sortedGroups="sortedGroups"
        :reload="reloadPickedComic" />
    </template>

    <div v-if="store.pickedComic !== undefined" class="flex p-2 pt-0">
      <img class="w-24 mr-4 object-cover" :src="store.pickedComic.cover" alt="" />
      <div class="flex flex-col h-full">
        <span class="font-bold text-lg line-clamp-2">
          {{ store.pickedComic.title }}
          {{ store.pickedComic.subtitle ? `(${store.pickedComic.subtitle})` : '' }}
        </span>
        <span v-if="store.pickedComic.authors.length !== 0" class="text-red">
          作者：{{ store.pickedComic.authors.join(', ') }}
        </span>
        <span v-if="store.pickedComic.genres.length !== 0" class="text-gray">
          类型：{{ store.pickedComic.genres.join(' ') }}
        </span>
        <IconButton
          v-if="store.pickedComic.isDownloaded"
          class="mt-auto mr-auto"
          title="打开下载目录"
          @click="showComicDownloadDirInFileManager">
          <PhFolderOpen :size="24" />
        </IconButton>
      </div>
    </div>
  </div>
</template>
