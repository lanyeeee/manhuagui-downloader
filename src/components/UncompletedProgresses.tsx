import { ProgressData } from '../types.ts'
import { useEffect, useMemo, useRef, useState } from 'react'
import {
  CheckOutlined,
  ClockCircleOutlined,
  DeleteOutlined,
  ExclamationCircleOutlined,
  LoadingOutlined,
  PauseOutlined,
  RightOutlined,
} from '@ant-design/icons'
import { commands, DownloadTaskState } from '../bindings.ts'
import SelectionArea, { SelectionEvent, useSelection } from '@viselect/react'
import { Dropdown, MenuProps, Progress } from 'antd'

interface Props {
  progresses: Map<number, ProgressData>
}

function UncompletedProgresses({ progresses }: Props) {
  // 已框选选到的下载任务的章节id
  const [selectedIds, setSelectedIds] = useState<Set<number>>(new Set())

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
    setSelectedIds((prev) => {
      const next = new Set(prev)
      extractIds(added).forEach((id) => next.add(id))
      extractIds(removed).forEach((id) => next.delete(id))
      console.log(`added: ${added}, removed: ${removed}`)
      return next
    })
  }

  function unselectAll({ event, selection }: SelectionEvent) {
    if (!event?.ctrlKey && !event?.metaKey) {
      selection.clearSelection()
      setSelectedIds(new Set())
    }
  }

  return (
    <SelectionArea
      className="h-full container selection-container flex flex-col"
      selectables=".selectable"
      features={{ deselectOnBlur: true }}
      onMove={updateSelectedIds}
      onStart={unselectAll}>
      <span className="mr-auto select-none">左键拖动进行框选，右键打开菜单，双击暂停/继续</span>
      <Progresses progresses={progresses} selectedIds={selectedIds} setSelectedIds={setSelectedIds} />
    </SelectionArea>
  )
}

interface ProgressesProps {
  progresses: Map<number, ProgressData>
  selectedIds: Set<number>
  setSelectedIds: (value: Set<number> | ((prev: Set<number>) => Set<number>)) => void
}

function Progresses({ progresses, selectedIds, setSelectedIds }: ProgressesProps) {
  const selection = useSelection()
  const selectableRefs = useRef<(HTMLDivElement | null)[]>([])

  const uncompletedProgresses = useMemo<[number, ProgressData][]>(
    () =>
      Array.from(progresses.entries())
        .filter(([, { state }]) => state !== 'Completed' && state !== 'Cancelled')
        .sort((a, b) => {
          return b[1].totalImgCount - a[1].totalImgCount
        }),
    [progresses],
  )

  // 清理selectedId中已经下载完成的章节
  useEffect(() => {
    setSelectedIds((prev) => {
      const uncompletedIds = new Set(uncompletedProgresses.map(([chapterId]) => chapterId))
      return new Set([...prev].filter((id) => uncompletedIds.has(id)))
    })
  }, [setSelectedIds, uncompletedProgresses])

  async function onProgressDoubleClick(state: DownloadTaskState, chapterId: number) {
    if (state === 'Downloading' || state === 'Pending') {
      const result = await commands.pauseDownloadTask(chapterId)
      if (result.status === 'error') {
        console.error(result.error)
      }
    } else {
      const result = await commands.resumeDownloadTask(chapterId)
      if (result.status === 'error') {
        console.error(result.error)
      }
    }
  }

  function onProgressContextMenu(chapterId: number) {
    setSelectedIds((prev) => {
      const next = new Set(prev)
      if (!prev.has(chapterId)) {
        next.clear()
        next.add(chapterId)
      }
      return next
    })
  }

  const dropdownOptions: MenuProps['items'] = [
    {
      label: '全选',
      key: 'check all',
      icon: <CheckOutlined />,
      onClick: () => {
        if (selection === undefined) {
          return
        }
        const selectables = selectableRefs.current.filter((el): el is HTMLDivElement => el !== null)
        selection?.select(selectables)
      },
    },
    {
      label: '继续',
      key: 'resume',
      icon: <RightOutlined />,
      onClick: () => {
        selectedIds.forEach(async (chapterId) => {
          const result = await commands.resumeDownloadTask(chapterId)
          if (result.status === 'error') {
            console.error(result.error)
          }
        })
      },
    },
    {
      label: '暂停',
      key: 'pause',
      icon: <PauseOutlined />,
      onClick: () => {
        selectedIds.forEach(async (chapterId) => {
          console.log('pause', chapterId)
          const result = await commands.pauseDownloadTask(chapterId)
          if (result.status === 'error') {
            console.error(result.error)
          }
        })
      },
    },
    {
      label: '取消',
      key: 'cancel',
      icon: <DeleteOutlined />,
      onClick: () => {
        selectedIds.forEach(async (chapterId) => {
          console.log('cancel', chapterId)
          const result = await commands.cancelDownloadTask(chapterId)
          if (result.status === 'error') {
            console.error(result.error)
          }
        })
      },
    },
  ]

  return (
    <Dropdown className="select-none" menu={{ items: dropdownOptions }} trigger={['contextMenu']}>
      <div className="h-full select-none">
        {uncompletedProgresses.map(([chapterId, progressData], index) => (
          <div
            ref={(el) => (selectableRefs.current[index] = el)}
            className={`selectable p-3 mb-2 rounded-lg ${selectedIds.has(chapterId) ? 'selected shadow-md' : 'hover:bg-gray-1'}`}
            key={chapterId}
            data-key={chapterId}
            onDoubleClick={() => onProgressDoubleClick(progressData.state, chapterId)}
            onContextMenu={() => onProgressContextMenu(chapterId)}>
            <DownloadingProgresses progressData={progressData} />
          </div>
        ))}
      </div>
    </Dropdown>
  )
}

function DownloadingProgresses({ progressData }: { progressData: ProgressData }) {
  function stateToStatus(state: DownloadTaskState): 'normal' | 'exception' | 'active' | 'success' {
    if (state === 'Downloading') {
      return 'active'
    } else if (state === 'Completed') {
      return 'success'
    } else if (state === 'Failed') {
      return 'exception'
    } else {
      return 'normal'
    }
  }

  function stateToColorClass(state: DownloadTaskState) {
    if (state === 'Downloading') {
      return 'text-blue-500'
    } else if (state === 'Pending') {
      return 'text-gray-500'
    } else if (state === 'Paused') {
      return 'text-yellow-500'
    } else if (state === 'Failed') {
      return 'text-red-500'
    } else if (state === 'Completed') {
      return 'text-green-500'
    } else if (state === 'Cancelled') {
      return 'text-stone-500'
    }

    return ''
  }

  function stateToColorHex(state: DownloadTaskState) {
    if (state === 'Downloading') {
      return '#2B7FFF'
    } else if (state === 'Pending') {
      return '#6A7282'
    } else if (state === 'Paused') {
      return '#F0B100'
    } else if (state === 'Failed') {
      return '#FB2C36'
    } else if (state === 'Completed') {
      return '#00C950'
    } else if (state === 'Cancelled') {
      return '#79716B'
    }

    return ''
  }

  const started = !isNaN(progressData.percentage)
  const color = stateToColorHex(progressData.state)
  const colorClass = stateToColorClass(progressData.state)

  return (
    <>
      <div className="grid grid-cols-[1fr_1fr_1fr]">
        <div className="text-ellipsis whitespace-nowrap overflow-hidden" title={progressData.chapterInfo.comicTitle}>
          {progressData.chapterInfo.comicTitle}
        </div>
        <div className="text-ellipsis whitespace-nowrap overflow-hidden" title={progressData.chapterInfo.groupName}>
          {progressData.chapterInfo.groupName}
        </div>
        <div className="text-ellipsis whitespace-nowrap overflow-hidden" title={progressData.chapterInfo.chapterTitle}>
          {progressData.chapterInfo.chapterTitle}
        </div>
      </div>
      <div className={`flex ${colorClass}`}>
        {progressData.state === 'Downloading' && <LoadingOutlined />}
        {progressData.state === 'Pending' && <ClockCircleOutlined />}
        {progressData.state === 'Paused' && <PauseOutlined />}
        {progressData.state === 'Failed' && <ExclamationCircleOutlined />}
        {!started && <div className="ml-auto">{progressData.indicator}</div>}
        {started && (
          <Progress
            className="ml-2"
            strokeColor={color}
            status={stateToStatus(progressData.state)}
            percent={progressData.percentage}
            format={() => <div className={colorClass}>{progressData.indicator}</div>}
          />
        )}
      </div>
    </>
  )
}

export default UncompletedProgresses
