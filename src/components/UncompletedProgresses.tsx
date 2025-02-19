import { ProgressData } from '../types.ts'
import { useEffect, useMemo, useRef, useState } from 'react'
import { CheckOutlined, DeleteOutlined, PauseOutlined, RightOutlined } from '@ant-design/icons'
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
      className="h-full container selection-container"
      selectables=".selectable"
      features={{ deselectOnBlur: true }}
      onMove={updateSelectedIds}
      onStart={unselectAll}>
      <span className="ml-auto select-none">左键拖动进行框选，右键打开菜单，双击暂停/继续</span>
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
    } else if (state === 'Paused') {
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
        {uncompletedProgresses.map(([chapterId, { state, chapterInfo, percentage, indicator }], index) => (
          <div
            ref={(el) => (selectableRefs.current[index] = el)}
            className={`grid grid-cols-[1fr_1fr_2fr] selectable ${selectedIds.has(chapterId) ? 'selected' : 'hover:bg-black/10'}`}
            key={chapterId}
            data-key={chapterId}
            onDoubleClick={() => onProgressDoubleClick(state, chapterId)}
            onContextMenu={() => onProgressContextMenu(chapterId)}>
            <span className="mb-1! text-ellipsis whitespace-nowrap overflow-hidden" title={chapterInfo.comicTitle}>
              {chapterInfo.comicTitle}
            </span>
            <span className="mb-1! text-ellipsis whitespace-nowrap overflow-hidden" title={chapterInfo.chapterTitle}>
              {chapterInfo.chapterTitle}
            </span>
            <DownloadingProgresses state={state} percentage={percentage} indicator={indicator} />
          </div>
        ))}
      </div>
    </Dropdown>
  )
}

interface DownloadingProgressesProps {
  state: DownloadTaskState
  percentage: number
  indicator: string
}

function DownloadingProgresses({ state, percentage, indicator }: DownloadingProgressesProps) {
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

  if (isNaN(percentage)) {
    return <span>{indicator}</span>
  }

  return <Progress status={stateToStatus(state)} percent={percentage} format={() => indicator} />
}

export default UncompletedProgresses
