import { Comic, commands, Config, events } from '../bindings.ts'
import { CurrentTabName } from '../types.ts'
import { useEffect, useMemo, useRef, useState } from 'react'
import { App as AntdApp, Button, Input, Pagination } from 'antd'
import DownloadedComicCard from '../components/DownloadedComicCard.tsx'
import { MessageInstance } from 'antd/es/message/interface'
import { open } from '@tauri-apps/plugin-dialog'
import { revealItemInDir } from '@tauri-apps/plugin-opener'

interface ProgressData {
  comicTitle: string
  current: number
  total: number
}

interface Props {
  config: Config
  setConfig: (value: Config | undefined | ((prev: Config | undefined) => Config | undefined)) => void
  setPickedComic: (value: Comic | undefined) => void
  currentTabName: CurrentTabName
  setCurrentTabName: (currentTabName: CurrentTabName) => void
}

function DownloadedPane({ config, setConfig, setPickedComic, currentTabName, setCurrentTabName }: Props) {
  const { message, notification } = AntdApp.useApp()

  const [downloadedComics, setDownloadedComics] = useState<Comic[]>([])
  const [downloadedPageNum, setDownloadedPageNum] = useState<number>(1)
  const progresses = useRef<Map<string, ProgressData>>(new Map())

  const showingDownloadedComics = useMemo<Comic[]>(() => {
    const PAGE_SIZE = 20
    const start = (downloadedPageNum - 1) * PAGE_SIZE
    const end = downloadedPageNum * PAGE_SIZE
    return downloadedComics.slice(start, end)
  }, [downloadedComics, downloadedPageNum])

  useEffect(() => {
    if (currentTabName !== 'downloaded') {
      return
    }

    commands.getDownloadedComics().then(async (result) => {
      if (result.status === 'error') {
        notification.error({ message: '获取本地库存失败', description: result.error, duration: 0 })
        return
      }

      setDownloadedComics(result.data)
    })
  }, [currentTabName, notification])

  const messageRef = useRef<MessageInstance>(message)
  useEffect(() => {
    messageRef.current = message
  }, [message])

  useEffect(() => {
    let mounted = true
    let unListenExportCbzEvent: () => void | undefined
    let unListenExportPdfEvent: () => void | undefined

    events.exportCbzEvent
      .listen(async ({ payload: exportCbzEvent }) => {
        if (exportCbzEvent.event === 'Start') {
          const { uuid, comicTitle, total } = exportCbzEvent.data
          progresses.current.set(uuid, { comicTitle, current: 0, total })
          messageRef.current.loading({ key: uuid, content: `${comicTitle} 正在导出cbz(0/${total})`, duration: 0 })
        } else if (exportCbzEvent.event === 'Progress') {
          const { uuid, current } = exportCbzEvent.data
          const progressData = progresses.current.get(uuid)
          if (progressData === undefined) {
            return
          }
          progresses.current.set(uuid, { ...progressData, current })
          messageRef.current.loading({
            key: uuid,
            content: `${progressData.comicTitle} 正在导出cbz(${current}/${progressData.total})`,
            duration: 0,
          })
        } else if (exportCbzEvent.event === 'End') {
          const { uuid } = exportCbzEvent.data
          const progressData = progresses.current.get(uuid)
          if (progressData === undefined) {
            return
          }
          messageRef.current.success({
            key: uuid,
            content: `${progressData.comicTitle} 导出cbz完成(${progressData.total}/${progressData.total})`,
          })
          progresses.current.delete(uuid)
        }
      })
      .then((unListenFn) => {
        if (mounted) {
          unListenExportCbzEvent = unListenFn
        } else {
          unListenFn()
        }
      })

    events.exportPdfEvent
      .listen(async ({ payload: exportPdfEvent }) => {
        if (exportPdfEvent.event === 'CreateStart') {
          const { uuid, comicTitle, total } = exportPdfEvent.data
          progresses.current.set(uuid, { comicTitle, current: 0, total })
          messageRef.current.loading({ key: uuid, content: `${comicTitle} 正在导出pdf(0/${total})`, duration: 0 })
        } else if (exportPdfEvent.event === 'CreateProgress') {
          const { uuid, current } = exportPdfEvent.data
          const progressData = progresses.current.get(uuid)
          if (progressData === undefined) {
            return
          }
          progresses.current.set(uuid, { ...progressData, current })
          messageRef.current.loading({
            key: uuid,
            content: `${progressData.comicTitle} 正在导出pdf(${current}/${progressData.total})`,
            duration: 0,
          })
        } else if (exportPdfEvent.event === 'CreateEnd') {
          const { uuid } = exportPdfEvent.data
          const progressData = progresses.current.get(uuid)
          if (progressData === undefined) {
            return
          }
          messageRef.current.success({
            key: uuid,
            content: `${progressData.comicTitle} 导出pdf完成(${progressData.total}/${progressData.total})`,
          })
          progresses.current.delete(uuid)
        } else if (exportPdfEvent.event === 'MergeStart') {
          const { uuid, comicTitle, total } = exportPdfEvent.data
          progresses.current.set(uuid, { comicTitle, current: 0, total })
          messageRef.current.loading({ key: uuid, content: `${comicTitle} 正在合并pdf(0/${total})`, duration: 0 })
        } else if (exportPdfEvent.event === 'MergeProgress') {
          const { uuid, current } = exportPdfEvent.data
          const progressData = progresses.current.get(uuid)
          if (progressData === undefined) {
            return
          }
          progresses.current.set(uuid, { ...progressData, current })
          messageRef.current.loading({
            key: uuid,
            content: `${progressData.comicTitle} 正在合并pdf(${current}/${progressData.total})`,
            duration: 0,
          })
        } else if (exportPdfEvent.event === 'MergeEnd') {
          const { uuid } = exportPdfEvent.data
          const progressData = progresses.current.get(uuid)
          if (progressData === undefined) {
            return
          }
          messageRef.current.success({
            key: uuid,
            content: `${progressData.comicTitle} 合并pdf完成(${progressData.total}/${progressData.total})`,
          })
          progresses.current.delete(uuid)
        }
      })
      .then((unListenFn) => {
        if (mounted) {
          unListenExportPdfEvent = unListenFn
        } else {
          unListenFn()
        }
      })

    return () => {
      mounted = false
      unListenExportCbzEvent?.()
      unListenExportPdfEvent?.()
    }
  }, [])

  async function selectExportDir() {
    const selectedDirPath = await open({ directory: true })
    if (selectedDirPath === null) {
      return
    }
    setConfig((prev) => {
      if (prev === undefined) {
        return prev
      }
      return { ...prev, exportDir: selectedDirPath }
    })
  }

  return (
    <div className="h-full flex flex-col overflow-auto">
      <div className="flex gap-col-1">
        <Input value={config.exportDir} prefix="导出目录" size="small" readOnly onClick={selectExportDir} />
        <Button size="small" onClick={() => revealItemInDir(config.exportDir)}>
          打开目录
        </Button>
      </div>
      <div className="h-full flex flex-col gap-row-1 overflow-auto">
        <div className="h-full flex flex-col gap-row-2 overflow-auto p-2">
          {showingDownloadedComics.map((comic) => (
            <DownloadedComicCard
              key={comic.id}
              comic={comic}
              setPickedComic={setPickedComic}
              setCurrentTabName={setCurrentTabName}
            />
          ))}
        </div>
        <Pagination
          current={downloadedPageNum}
          pageSize={20}
          total={downloadedComics.length}
          showSizeChanger={false}
          simple
          onChange={(pageNum) => setDownloadedPageNum(pageNum)}
        />
      </div>
    </div>
  )
}

export default DownloadedPane
