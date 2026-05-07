<script setup lang="ts">
import { commands } from '../bindings.ts'
import ComicCard from '../components/ComicCard.vue'
import { computed, ref, watch } from 'vue'
import { useStore } from '../store.ts'
import { NPagination } from 'naive-ui'

const store = useStore()

const currentPage = ref<number>(1)

const pageCount = computed(() => {
  const PAGE_SIZE = 20
  if (store.getFavoriteResult === undefined) {
    return 1
  }
  return Math.ceil(store.getFavoriteResult.total / PAGE_SIZE)
})

async function getFavourite(pageNum: number) {
  currentPage.value = pageNum

  const result = await commands.getFavorite(pageNum)
  if (result.status === 'error') {
    console.error(result.error)
    return
  }

  store.getFavoriteResult = result.data
}

watch(
  () => store.userProfile,
  async () => {
    if (store.userProfile === undefined) {
      store.getFavoriteResult = undefined
      return
    }
    await getFavourite(1)
  },
  { immediate: true },
)
</script>

<template>
  <div class="h-full flex flex-col">
    <div v-if="store.getFavoriteResult !== undefined" class="h-full flex flex-col gap-row-1 overflow-auto">
      <div class="h-full flex flex-col gap-row-2 overflow-auto p-2">
        <ComicCard
          v-for="comic in store.getFavoriteResult.comics"
          :key="comic.id"
          :comicId="comic.id"
          :comicTitle="comic.title"
          :comicCover="comic.cover"
          :comicLastUpdateTime="comic.lastUpdate"
          :comicLastReadTime="comic.lastRead"
          :comic-downloaded="comic.isDownloaded" />
      </div>
      <n-pagination
        class="p-2 mt-auto"
        :page="currentPage"
        :pageCount="pageCount"
        @update:page="getFavourite($event)" />
    </div>
  </div>
</template>
