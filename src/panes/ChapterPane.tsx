import {
  App as AntdApp,
  Button,
  Card,
  Checkbox,
  CheckboxProps,
  Divider,
  Dropdown,
  Empty,
  MenuProps,
  Tabs,
  TabsProps,
} from 'antd'
import { ChapterInfo, Comic, commands } from '../bindings.ts'
import { useEffect, useMemo, useState } from 'react'
import SelectionArea, { SelectionEvent } from '@viselect/react'

interface Props {
  pickedComic: Comic | undefined
  setPickedComic: (update: (prevComic: Comic | undefined) => Comic | undefined) => void
}

function ChapterPane({ pickedComic, setPickedComic }: Props) {
  const { message } = AntdApp.useApp()
  // 按章节数排序的分组
  const sortedGroups = useMemo<[string, ChapterInfo[]][] | undefined>(() => {
    const groups = pickedComic?.groups
    if (groups === undefined) {
      return
    }
    return Object.entries(groups).sort((a, b) => {
      return b[1].length - a[1].length
    })
  }, [pickedComic?.groups])
  // 第一个group的名字
  const firstGroupName = sortedGroups?.[0]?.[0] ?? '单话'
  // 当前tab的分组名
  const [currentGroupName, setCurrentGroupName] = useState<string>(firstGroupName)

  // 所有章节
  const chapterInfos = useMemo<ChapterInfo[] | undefined>(() => {
    const groups = pickedComic?.groups
    if (groups === undefined) {
      return
    }

    return Object.values(groups).flat()
  }, [pickedComic?.groups])

  // 已勾选的章节id
  const [checkedIds, setCheckedIds] = useState<Set<number>>(new Set())
  // 已选中(被框选选到)的章节id
  const [selectedIds, setSelectedIds] = useState<Set<number>>(new Set())
  // 如果漫画变了，清空勾选和选中状态
  useEffect(() => {
    setCheckedIds(new Set())
    setSelectedIds(new Set())
    setCurrentGroupName(firstGroupName)
  }, [firstGroupName, pickedComic?.id])

  // 下载勾选的章节
  async function downloadChapters() {
    if (pickedComic === undefined) {
      message.error('请先选择漫画')
      return
    }
    // 创建下载任务前，先创建元数据
    const saveMetadataResult = await commands.saveMetadata(pickedComic)
    if (saveMetadataResult.status === 'error') {
      console.error(saveMetadataResult.error)
      return
    }
    // 下载没有下载过的且已勾选的章节
    const chapterToDownload = chapterInfos?.filter((c) => c.isDownloaded === false && checkedIds.has(c.chapterId))
    if (chapterToDownload === undefined) {
      return
    }
    const result = await commands.downloadChapters(chapterToDownload)
    if (result.status === 'error') {
      console.error(result.error)
      return
    }
    // 把已下载的章节从已勾选的章节id中移除
    setCheckedIds((prev) => new Set([...prev].filter((id) => !chapterToDownload.map((c) => c.chapterId).includes(id))))
    // 更新pickedComic，将已下载的章节标记为已下载
    setPickedComic((prev) => {
      if (prev === undefined) {
        return prev
      }
      const next = { ...prev }
      for (const downloadedChapter of chapterToDownload) {
        const chapter = Object.values(next.groups)
          .flat()
          .find((c) => c.chapterId === downloadedChapter.chapterId)
        if (chapter !== undefined) {
          chapter.isDownloaded = true
        }
      }
      return next
    })
  }

  // 重新加载选中的漫画
  async function reloadPickedComic() {
    if (pickedComic === undefined) {
      return
    }

    const result = await commands.getComic(pickedComic.id)
    if (result.status === 'error') {
      console.error(result.error)
      return
    }

    setPickedComic(() => result.data)
  }

  return (
    <div className="h-full flex flex-col">
      <div className="flex flex-justify-around select-none">
        <span>总章数：{chapterInfos?.length}</span>
        <Divider type="vertical" />
        <span>已下载：{chapterInfos?.filter((c) => c.isDownloaded).length}</span>
        <Divider type="vertical" />
        <span>已勾选：{checkedIds.size}</span>
      </div>
      <div className="flex justify-between select-none">
        左键拖动进行框选，右键打开菜单
        <Button className="w-1/6" disabled={pickedComic === undefined} size="small" onClick={reloadPickedComic}>
          刷新
        </Button>
        <Button
          className="w-1/4"
          disabled={pickedComic === undefined}
          type="primary"
          size="small"
          onClick={downloadChapters}>
          下载勾选章节
        </Button>
      </div>
      <ChapterTabs
        pickedComic={pickedComic}
        sortedGroups={sortedGroups}
        setCheckedIds={setCheckedIds}
        selectedIds={selectedIds}
        setSelectedIds={setSelectedIds}
        checkedIds={checkedIds}
        currentGroupName={currentGroupName}
        setCurrentGroupName={setCurrentGroupName}
      />
      {pickedComic !== undefined && (
        <Card className="cursor-auto m-0! rounded-none" styles={{ body: { padding: '0.25rem' } }}>
          <div className="flex">
            <img className="w-24" src={pickedComic.cover} alt="" />
            <div className="flex flex-col h-full">
              <span className="font-bold text-xl line-clamp-3">
                {pickedComic.title}
                {pickedComic.subtitle && `(${pickedComic.subtitle})`}
              </span>
              <span className="text-red">作者：{pickedComic.authors.join(', ')}</span>
              <span className="text-gray">类型：{pickedComic.genres.join(' ')}</span>
            </div>
          </div>
        </Card>
      )}
    </div>
  )
}

interface ChapterTabsProps {
  pickedComic: Comic | undefined
  sortedGroups?: [string, ChapterInfo[]][]
  setCheckedIds: (value: ((prevState: Set<number>) => Set<number>) | Set<number>) => void
  selectedIds: Set<number>
  setSelectedIds: (value: ((prevState: Set<number>) => Set<number>) | Set<number>) => void
  checkedIds: Set<number>
  currentGroupName: string
  setCurrentGroupName: (value: string) => void
}

function ChapterTabs({
  pickedComic,
  sortedGroups,
  setCheckedIds,
  selectedIds,
  setSelectedIds,
  checkedIds,
  currentGroupName,
  setCurrentGroupName,
}: ChapterTabsProps) {
  // 当前分组
  const currentGroup = pickedComic?.groups[currentGroupName]

  const items = useMemo<TabsProps['items']>(() => {
    // 提取章节id
    function extractIds(elements: Element[]): number[] {
      return elements
        .map((element) => element.getAttribute('data-key'))
        .filter(Boolean)
        .map(Number)
        .filter((id) => {
          const chapterInfo = currentGroup?.find((chapter) => chapter.chapterId === id)
          return chapterInfo && chapterInfo.isDownloaded === false
        })
    }

    // 取消所有已选中(被框选选到)的章节
    function unselectAll({ event, selection }: SelectionEvent) {
      if (!event?.ctrlKey && !event?.metaKey) {
        selection.clearSelection()
        setSelectedIds(new Set())
      }
    }

    // 更新已选中(被框选选到)的章节id
    function updateSelectedIds({
      store: {
        changed: { added, removed },
      },
    }: SelectionEvent) {
      setSelectedIds((prev) => {
        const next = new Set(prev)
        extractIds(added).forEach((id) => next.add(id))
        extractIds(removed).forEach((id) => next.delete(id))
        console.log(`added: ${extractIds(added)}, removed: ${extractIds(removed)}`)
        return next
      })
    }

    if (sortedGroups === undefined) {
      return []
    }

    const onCheckboxChange: CheckboxProps['onChange'] = (e) => {
      setCheckedIds((prev) => {
        const next = new Set(prev)
        const id = e.target.value
        if (e.target.checked) {
          next.add(id)
        } else {
          next.delete(id)
        }
        return next
      })
    }

    const dropdownOptions: MenuProps['items'] = [
      {
        label: '勾选',
        key: 'check',
        onClick: () =>
          // 将框选选到的章节id加入已勾选的章节id中
          setCheckedIds((prev) => {
            const next = new Set(prev)
            selectedIds.forEach((id) => next.add(id))
            return next
          }),
      },
      {
        label: '取消勾选',
        key: 'uncheck',
        onClick: () =>
          // 将框选选到的章节id从已勾选的章节id中移除
          setCheckedIds((prev) => new Set([...prev].filter((id) => !selectedIds.has(id)))),
      },
      {
        label: '全选',
        key: 'check all',
        onClick: () =>
          // 将当前分组中未下载的章节id加入已勾选的章节id中
          setCheckedIds((prev) => {
            const next = new Set(prev)
            currentGroup?.filter((c) => c.isDownloaded === false).forEach((c) => next.add(c.chapterId))
            return next
          }),
      },
      {
        label: '取消全选',
        key: 'uncheck all',
        onClick: () =>
          // 将当前分组中未下载的章节id从已勾选的章节id中移除
          setCheckedIds(
            (prev) => new Set([...prev].filter((id) => !currentGroup?.map((c) => c.chapterId).includes(id))),
          ),
      },
    ]

    return sortedGroups.map(([groupName, chapters]) => ({
      key: groupName,
      label: groupName,
      children: (
        <Dropdown menu={{ items: dropdownOptions }} trigger={['contextMenu']}>
          <div className="h-full flex flex-col gap-row-1 overflow-auto">
            <SelectionArea
              className="container selection-container h-full"
              selectables=".selectable"
              features={{ deselectOnBlur: true }}
              onMove={updateSelectedIds}
              onStart={unselectAll}>
              <div className="grid grid-cols-3 gap-1.5 w-full mb-3">
                {chapters.map((chapter) => (
                  <div
                    className={`${selectedIds.has(chapter.chapterId) ? 'selected' : ''} ${chapter.isDownloaded ? 'downloaded' : ''} selectable`}
                    key={chapter.chapterId}
                    data-key={chapter.chapterId}>
                    <Checkbox
                      value={chapter.chapterId}
                      checked={checkedIds.has(chapter.chapterId)}
                      disabled={chapter.isDownloaded === true}
                      onChange={onCheckboxChange}>
                      <span title={chapter.chapterTitle}>{chapter.chapterTitle}</span>
                    </Checkbox>
                  </div>
                ))}
              </div>
            </SelectionArea>
          </div>
        </Dropdown>
      ),
    }))
  }, [sortedGroups, setSelectedIds, setCheckedIds, selectedIds, currentGroup, checkedIds])

  if (pickedComic === undefined) {
    return <Empty description="请先进行漫画搜索" />
  }

  return (
    <Tabs
      key={pickedComic.id}
      activeKey={currentGroupName}
      className="flex-1 overflow-auto select-none"
      size="small"
      items={items}
      onChange={setCurrentGroupName}
    />
  )
}

export default ChapterPane
