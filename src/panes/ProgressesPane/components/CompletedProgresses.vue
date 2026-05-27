<script setup lang="ts">
import { computed } from 'vue'
import { useStore } from '../../../store.ts'

const store = useStore()

const completedProgresses = computed(() => {
  return Array.from(store.progresses.entries())
    .filter(([, { state }]) => state === 'Completed')
    .sort((a, b) => {
      return b[1].totalImgCount - a[1].totalImgCount
    })
})
</script>

<template>
  <div class="h-full flex flex-col gap-row-2 px-1">
    <div
      v-for="[chapterId, { chapterInfo }] in completedProgresses"
      :key="chapterId"
      class="grid grid-cols-[1fr_1fr_1fr] py-2 px-4 bg-gray-100 rounded-lg">
      <span class="text-ellipsis whitespace-nowrap overflow-hidden" :title="chapterInfo.comicTitle">
        {{ chapterInfo.comicTitle }}
      </span>
      <span class="text-ellipsis whitespace-nowrap overflow-hidden" :title="chapterInfo.groupName">
        {{ chapterInfo.groupName }}
      </span>
      <span class="text-ellipsis whitespace-nowrap overflow-hidden" :title="chapterInfo.chapterTitle">
        {{ chapterInfo.chapterTitle }}
      </span>
    </div>
  </div>
</template>
