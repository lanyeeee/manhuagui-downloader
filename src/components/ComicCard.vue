<script setup lang="ts">
import { commands } from '../bindings.ts'
import { useStore } from '../store.ts'

const store = useStore()

const props = defineProps<{
  comicId: number
  comicTitle: string
  comicCover: string
  comicSubtitle?: string | null
  comicAuthors?: string[]
  comicGenres?: string[]
  comicLastUpdateTime?: string
  comicLastReadTime?: string
}>()

async function pickComic() {
  const result = await commands.getComic(props.comicId)
  if (result.status === 'error') {
    console.error(result.error)
    return
  }

  store.pickedComic = result.data
  store.currentTabName = 'chapter'
}
</script>

<template>
  <n-card content-style="padding: 0.25rem;" hoverable>
    <div class="flex h-full">
      <img
        class="w-24 object-cover mr-4 cursor-pointer transition-transform duration-200 hover:scale-106"
        :src="comicCover"
        alt=""
        @click="pickComic" />
      <div class="flex flex-col h-full">
        <span
          class="font-bold text-lg line-clamp-3 cursor-pointer transition-colors duration-200 hover:text-blue-5"
          @click="pickComic">
          {{ comicTitle }} {{ comicSubtitle && `(${comicSubtitle})` }}
        </span>
        <span v-if="comicAuthors !== undefined" class="text-red">作者：{{ comicAuthors.join(', ') }}</span>
        <span v-if="comicGenres !== undefined" class="text-black">类型：{{ comicGenres.join(' ') }}</span>
        <span v-if="comicLastUpdateTime !== undefined" class="text-gray">上次更新：{{ comicLastUpdateTime }}</span>
        <span v-if="comicLastReadTime !== undefined" class="text-gray">上次阅读：{{ comicLastReadTime }}</span>
      </div>
    </div>
  </n-card>
</template>
