<script setup lang="ts">
import { Comic, commands } from '../../../bindings.ts'
import { computed } from 'vue'
import { useStore } from '../../../store.ts'
import { NCard } from 'naive-ui'
import IconButton from '../../../components/IconButton.vue'
import { PhFilePdf, PhFileZip, PhFolderOpen } from '@phosphor-icons/vue'

interface GroupInfo {
  name: string
  downloaded: number
  total: number
}

const props = defineProps<{
  comic: Comic
}>()

const store = useStore()

const groupInfos = computed<GroupInfo[]>(() => {
  const groups = props.comic.groups

  const infos = Object.values(groups).map((chapterInfos) => {
    const groupInfo: GroupInfo = {
      name: chapterInfos[0].groupName,
      downloaded: chapterInfos.filter((chapterInfo) => chapterInfo.isDownloaded).length,
      total: chapterInfos.length,
    }
    return groupInfo
  })

  infos.sort((a, b) => b.total - a.total)
  return infos
})

function pickComic() {
  store.pickedComic = props.comic
  store.currentTabName = 'chapter'
}

async function exportCbz() {
  const result = await commands.exportCbz(props.comic)
  if (result.status === 'error') {
    console.error(result.error)
    return
  }
}

async function exportPdf() {
  const result = await commands.exportPdf(props.comic)
  if (result.status === 'error') {
    console.error(result.error)
    return
  }
}

async function showComicDownloadDirInFileManager() {
  const comicDownloadDir = props.comic.comicDownloadDir
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
  <n-card hoverable content-style="padding: 0.25rem;">
    <div class="flex">
      <img
        class="w-24 object-cover mr-4 cursor-pointer transition-transform duration-200 hover:scale-106"
        :src="comic.cover"
        alt=""
        @click="pickComic" />
      <div class="flex flex-col h-full w-full">
        <span
          class="font-bold text-lg line-clamp-2 cursor-pointer transition-colors duration-200 hover:text-blue-5"
          @click="pickComic">
          {{ comic.title }} {{ comic.subtitle ? `(${comic.subtitle})` : '' }}
        </span>

        <span v-if="comic.authors" class="text-red">作者：{{ comic.authors.join(', ') }}</span>
        <span v-if="comic.genres" class="text-black">类型：{{ comic.genres.join(' ') }}</span>
        <span v-for="groupInfo in groupInfos" :key="groupInfo.name" class="text-black">
          {{ groupInfo.name }}：{{ groupInfo.downloaded }}/{{ groupInfo.total }}
        </span>

        <div class="flex mt-auto gap-col-2">
          <IconButton title="打开下载目录" @click="showComicDownloadDirInFileManager">
            <PhFolderOpen :size="24" />
          </IconButton>

          <IconButton class="ml-auto" title="导出cbz" @click="exportCbz">
            <PhFileZip :size="24" />
          </IconButton>

          <IconButton title="导出pdf" @click="exportPdf">
            <PhFilePdf :size="24" />
          </IconButton>
        </div>
      </div>
    </div>
  </n-card>
</template>
