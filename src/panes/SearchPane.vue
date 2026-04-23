<script setup lang="ts">
import { commands, SearchResult } from '../bindings.ts'
import ComicCard from '../components/ComicCard.vue'
import FloatLabelInput from '../components/FloatLabelInput.vue'
import { useMessage } from 'naive-ui'
import { PhArrowRight, PhMagnifyingGlass } from '@phosphor-icons/vue'
import { computed, ref } from 'vue'
import { useStore } from '../store.ts'

const store = useStore()

const message = useMessage()

const searchInput = ref<string>('')
const comicIdInput = ref<string>('')
const currentPage = ref<number>(1)
const searchResult = ref<SearchResult>()
const searching = ref<boolean>(false)
const picking = ref<boolean>(false)

const pageCount = computed(() => {
  const PAGE_SIZE = 10

  if (searchResult.value === undefined) {
    return 1
  }

  return Math.ceil(searchResult.value.total / PAGE_SIZE)
})

async function search(keyword: string, pageNum: number) {
  currentPage.value = pageNum
  searching.value = true

  const result = await commands.search(keyword, pageNum)
  if (result.status === 'error') {
    console.error(result.error)
    searching.value = false
    return
  }

  searchResult.value = result.data
  searching.value = false
}

function getComicIdFromComicIdInput(): number | undefined {
  const comicIdString = comicIdInput.value.trim()
  const comicId = parseInt(comicIdString)
  if (!isNaN(comicId)) {
    return comicId
  }

  const regex = /\/comic\/(\d+)/
  const match = comicIdString.match(regex)
  if (match === null || match[1] === null) {
    return
  }
  return parseInt(match[1])
}

async function pickComic() {
  const comicId = getComicIdFromComicIdInput()

  if (comicId === undefined) {
    message.error('漫画ID格式错误，请输入正确的漫画ID或漫画链接')
    return
  }

  picking.value = true

  const result = await commands.getComic(comicId)
  if (result.status === 'error') {
    console.error(result.error)
    picking.value = false
    return
  }

  picking.value = false

  store.pickedComic = result.data
  store.currentTabName = 'chapter'
}
</script>

<template>
  <div class="h-full flex flex-col">
    <n-input-group class="box-border px-2 pt-2">
      <FloatLabelInput
        label="关键词"
        size="small"
        v-model:value="searchInput"
        clearable
        @keydown.enter="search(searchInput.trim(), 1)" />
      <n-button :loading="searching" type="primary" size="small" class="w-15%" @click="search(searchInput.trim(), 1)">
        <template #icon>
          <n-icon size="22">
            <PhMagnifyingGlass />
          </n-icon>
        </template>
      </n-button>
    </n-input-group>

    <n-input-group class="box-border px-2 pt-2">
      <FloatLabelInput
        label="漫画ID (链接也行)"
        size="small"
        v-model:value="comicIdInput"
        clearable
        @keydown.enter="pickComic" />
      <n-button :loading="picking" type="primary" size="small" class="w-15%" @click="pickComic">
        <template #icon>
          <n-icon size="22">
            <PhArrowRight />
          </n-icon>
        </template>
      </n-button>
    </n-input-group>

    <div v-if="searchResult !== undefined" class="flex flex-col overflow-auto">
      <div class="flex flex-col gap-row-2 overflow-auto p-2">
        <ComicCard
          v-for="comic in searchResult.comics"
          :key="comic.id"
          :comicId="comic.id"
          :comicTitle="comic.title"
          :comicCover="comic.cover"
          :comicSubtitle="comic.subtitle ?? undefined"
          :comicAuthors="comic.authors"
          :comicGenres="comic.genres"
          :comicLastUpdateTime="comic.updateTime" />
      </div>
    </div>

    <n-pagination
      v-if="searchResult !== undefined && searchResult.total > 0"
      class="p-2 mt-auto"
      :page="currentPage"
      :pageCount="pageCount"
      @update:page="search(searchInput.trim(), $event)" />
  </div>
</template>
