<script setup lang="tsx">
import { computed, defineComponent, nextTick, PropType, ref, useTemplateRef, watchEffect } from 'vue'
import { DropdownOption, NDropdown, NIcon, NProgress, ProgressProps } from 'naive-ui'
import { PartialSelectionOptions, SelectionArea, SelectionEvent } from '@viselect/vue'
import {
  PhCaretRight,
  PhChecks,
  PhClock,
  PhCloudArrowDown,
  PhPause,
  PhTrash,
  PhWarningCircle,
} from '@phosphor-icons/vue'
import { commands } from '../../../bindings.ts'
import { ProgressData } from '../../../types.ts'
import { useStore } from '../../../store.ts'

const store = useStore()

const selectionOptions: PartialSelectionOptions = {
  selectables: '.selectable',
  features: { deselectOnBlur: true },
  boundaries: '.uncompleted-progresses-selection-container',
}
const selectionAreaRef = useTemplateRef('selectionAreaRef')

const selectedIds = ref<Set<number>>(new Set())
const selectableRefs = useTemplateRef('selectableRefs')

const uncompletedProgresses = computed<[number, ProgressData][]>(() => {
  return Array.from(store.progresses.entries())
    .filter(([, { state }]) => state !== 'Completed')
    .sort((a, b) => {
      return b[1].totalImgCount - a[1].totalImgCount
    })
})

watchEffect(() => {
  // 只保留未完成的章节id
  const uncompletedIds = new Set(uncompletedProgresses.value.map(([chapterId]) => chapterId))
  for (const id of selectedIds.value) {
    if (!uncompletedIds.has(id)) {
      selectedIds.value.delete(id)
    }
  }
})

function extractIds(elements: Element[]): number[] {
  return elements
    .map((element) => element.getAttribute('data-key'))
    .filter(Boolean)
    .map(Number)
}

function updateSelectedIds({
  store: {
    changed: { added, removed },
  },
}: SelectionEvent) {
  extractIds(added).forEach((chapterId) => selectedIds.value.add(chapterId))
  extractIds(removed).forEach((chapterId) => selectedIds.value.delete(chapterId))
}

function unselectAll({ event, selection }: SelectionEvent) {
  if (!event?.ctrlKey && !event?.metaKey) {
    selection.clearSelection()
    selectedIds.value.clear()
  }
}

const dropdownX = ref<number>(0)
const dropdownY = ref<number>(0)
const dropdownShowing = ref<boolean>(false)
const dropdownOptions: DropdownOption[] = [
  {
    label: '全选',
    key: 'select-all',
    icon: () => (
      <NIcon size="20">
        <PhChecks />
      </NIcon>
    ),
    props: {
      onClick: () => {
        if (selectionAreaRef.value === undefined || selectableRefs.value === null) {
          dropdownShowing.value = false
          return
        }
        const selection = selectionAreaRef.value?.selection
        if (selection === undefined) {
          dropdownShowing.value = false
          return
        }
        selection.select(selectableRefs.value.map((ref) => ref?.$el))
        dropdownShowing.value = false
      },
    },
  },
  {
    label: '继续',
    key: 'resume',
    icon: () => (
      <NIcon size="20">
        <PhCaretRight />
      </NIcon>
    ),
    props: {
      onClick: () => {
        selectedIds.value.forEach(async (chapterId) => {
          const progressData = store.progresses.get(chapterId)
          if (progressData === undefined) {
            return
          }
          const { state, comic } = progressData
          if (state === 'Failed') {
            const result = await commands.createDownloadTask(comic, chapterId)
            if (result.status === 'error') {
              console.error(result.error)
            }
            return
          }

          const result = await commands.resumeDownloadTask(chapterId)
          if (result.status === 'error') {
            console.error(result.error)
          }
        })
        dropdownShowing.value = false
      },
    },
  },
  {
    label: '暂停',
    key: 'pause',
    icon: () => (
      <NIcon size="20">
        <PhPause />
      </NIcon>
    ),
    props: {
      onClick: () => {
        selectedIds.value.forEach(async (chapterId) => {
          const result = await commands.pauseDownloadTask(chapterId)
          if (result.status === 'error') {
            console.error(result.error)
          }
        })
        dropdownShowing.value = false
      },
    },
  },
  {
    label: '删除',
    key: 'cancel',
    icon: () => (
      <NIcon size="20">
        <PhTrash />
      </NIcon>
    ),
    props: {
      onClick: () => {
        selectedIds.value.forEach(async (chapterId) => {
          const result = await commands.deleteDownloadTask(chapterId)
          if (result.status === 'error') {
            console.error(result.error)
          }
        })
        dropdownShowing.value = false
      },
    },
  },
]

async function showDropdown(e: MouseEvent) {
  dropdownShowing.value = false
  await nextTick()
  dropdownShowing.value = true
  dropdownX.value = e.clientX
  dropdownY.value = e.clientY
}

const UncompletedProgress = defineComponent({
  name: 'UncompletedProgress',
  props: {
    p: {
      type: Object as PropType<ProgressData>,
      required: true,
    },
  },
  setup(props) {
    async function onDoubleClick() {
      if (props.p.state === 'Downloading' || props.p.state === 'Pending') {
        const result = await commands.pauseDownloadTask(props.p.chapterInfo.chapterId)
        if (result.status === 'error') {
          console.error(result.error)
        }
      } else if (props.p.state === 'Paused') {
        const result = await commands.resumeDownloadTask(props.p.chapterInfo.chapterId)
        if (result.status === 'error') {
          console.error(result.error)
        }
      } else {
        const progressData = store.progresses.get(props.p.chapterInfo.chapterId)
        if (progressData === undefined) {
          return
        }
        const { comic } = progressData
        const result = await commands.createDownloadTask(comic, props.p.chapterInfo.chapterId)
        if (result.status === 'error') {
          console.error(result.error)
        }
      }
    }

    function onContextMenu() {
      if (selectedIds.value.has(props.p.chapterInfo.chapterId)) {
        return
      }
      selectedIds.value.clear()
      selectedIds.value.add(props.p.chapterInfo.chapterId)
    }

    const progressStatus = computed<ProgressProps['status']>(() => {
      if (props.p.state === 'Completed') {
        return 'success'
      } else if (props.p.state === 'Paused') {
        return 'warning'
      } else if (props.p.state === 'Failed') {
        return 'error'
      } else {
        return 'default'
      }
    })

    const colorClass = computed<string>(() => {
      if (props.p.state === 'Downloading') {
        return 'text-blue-500'
      } else if (props.p.state === 'Pending') {
        return 'text-gray-500'
      } else if (props.p.state === 'Paused') {
        return 'text-yellow-500'
      } else if (props.p.state === 'Failed') {
        return 'text-red-500'
      } else if (props.p.state === 'Completed') {
        return 'text-green-500'
      }

      return ''
    })

    return () => (
      <div
        class={[
          'selectable p-3 mb-2 rounded-lg',
          selectedIds.value.has(props.p.chapterInfo.chapterId) ? 'selected shadow-md' : 'hover:bg-gray-1',
        ]}
        onDblclick={onDoubleClick}
        onContextmenu={onContextMenu}>
        <div class="grid grid-cols-[1fr_1fr_1fr]">
          <div class="text-ellipsis whitespace-nowrap overflow-hidden" title={props.p.chapterInfo.comicTitle}>
            {props.p.chapterInfo.comicTitle}
          </div>
          <div class="text-ellipsis whitespace-nowrap overflow-hidden" title={props.p.chapterInfo.groupName}>
            {props.p.chapterInfo.groupName}
          </div>
          <div class="text-ellipsis whitespace-nowrap overflow-hidden" title={props.p.chapterInfo.chapterTitle}>
            {props.p.chapterInfo.chapterTitle}
          </div>
        </div>
        <div class={`flex items-center mt-1 ${colorClass.value}`}>
          <NIcon class={[colorClass.value, 'mr-2']} size={20}>
            {props.p.state === 'Downloading' && <PhCloudArrowDown />}
            {props.p.state === 'Pending' && <PhClock />}
            {props.p.state === 'Paused' && <PhPause />}
            {props.p.state === 'Failed' && <PhWarningCircle />}
          </NIcon>
          {props.p.totalImgCount === 0 && <div class="ml-auto">{props.p.indicator}</div>}
          {props.p.totalImgCount !== 0 && (
            <NProgress
              class={colorClass.value}
              status={progressStatus.value}
              percentage={props.p.percentage}
              processing={props.p.state === 'Downloading'}>
              {props.p.indicator}
            </NProgress>
          )}
        </div>
      </div>
    )
  },
})
</script>

<template>
  <div class="uncompleted-progresses-selection-container h-full flex flex-col px-2" @contextmenu="showDropdown">
    <selection-area ref="selectionAreaRef" :options="selectionOptions" @move="updateSelectedIds" @start="unselectAll" />
    <span class="ml-auto select-none" @contextmenu="showDropdown">左键拖动进行框选，右键打开菜单，双击暂停/继续</span>
    <div class="h-full select-none">
      <UncompletedProgress
        ref="selectableRefs"
        v-for="[chapterId, p] in uncompletedProgresses"
        :key="chapterId"
        :data-key="chapterId"
        :p="p" />
    </div>
    <n-dropdown
      placement="bottom-start"
      trigger="manual"
      :x="dropdownX"
      :y="dropdownY"
      :options="dropdownOptions"
      :show="dropdownShowing"
      @clickoutside="dropdownShowing = false" />
  </div>
</template>

<style scoped>
.uncompleted-progresses-selection-container {
  @apply select-none overflow-auto;
}

.uncompleted-progresses-selection-container .selected {
  @apply bg-[rgb(204,232,255)];
}

:global(.selection-area) {
  @apply bg-[rgba(46,115,252,0.5)];
}
</style>
