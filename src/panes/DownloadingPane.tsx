import { App as AntdApp, Button, Input, Progress } from 'antd'
import { Config, events } from '../bindings.ts'
import { useEffect, useMemo, useRef, useState } from 'react'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import { open } from '@tauri-apps/plugin-dialog'

type ProgressData = {
  comicTitle: string
  chapterTitle: string
  current: number
  total: number
  percentage: number
  indicator: string
  retryAfter: number
}

interface Props {
  className?: string
  config: Config
  setConfig: (value: Config | undefined | ((prev: Config | undefined) => Config | undefined)) => void
}

function DownloadingPane({ className, config, setConfig }: Props) {
  const { notification } = AntdApp.useApp()
  const [progresses, setProgresses] = useState<Map<number, ProgressData>>(new Map())
  const [downloadSpeed, setDownloadSpeed] = useState<string>()
  const sortedProgresses = useMemo(
    () =>
      Array.from(progresses.entries()).sort((a, b) => {
        return b[1].total - a[1].total
      }),
    [progresses],
  )

  const notificationRef = useRef(notification)
  const progressesRef = useRef(progresses)

  useEffect(() => {
    notificationRef.current = notification
  }, [notification])

  useEffect(() => {
    progressesRef.current = progresses
  }, [progresses])

  useEffect(() => {
    let mounted = true
    let unListen: () => void | undefined

    events.downloadEvent
      .listen(({ payload: downloadEvent }) => {
        if (downloadEvent.event == 'ChapterPending') {
          console.log(downloadEvent)
          const { chapterId, comicTitle, chapterTitle } = downloadEvent.data
          const progressData: ProgressData = {
            comicTitle,
            chapterTitle,
            current: 0,
            total: 0,
            percentage: 0,
            indicator: '',
            retryAfter: 0,
          }
          setProgresses((prev) => new Map(prev).set(chapterId, progressData))
        } else if (downloadEvent.event == 'ChapterControlRisk') {
          const { chapterId, retryAfter } = downloadEvent.data
          setProgresses((prev) => {
            const progressData = prev.get(chapterId)
            if (progressData === undefined) {
              return prev
            }
            const next = new Map(prev)
            next.set(chapterId, { ...progressData, retryAfter })
            return new Map(next)
          })
        } else if (downloadEvent.event == 'ChapterStart') {
          const { chapterId, total } = downloadEvent.data
          setProgresses((prev) => {
            const progressData = prev.get(chapterId)
            if (progressData === undefined) {
              return prev
            }
            const next = new Map(prev)
            next.set(chapterId, { ...progressData, total })
            return new Map(next)
          })
        } else if (downloadEvent.event == 'ChapterEnd') {
          const { chapterId, errMsg } = downloadEvent.data
          setProgresses((prev) => {
            const progressData = prev.get(chapterId)
            if (progressData === undefined) {
              return prev
            }
            if (errMsg !== null) {
              notificationRef.current.error({
                message: `${progressData.comicTitle} - ${progressData.chapterTitle}下载章节失败`,
                description: errMsg,
                duration: 0,
              })
            }
            const next = new Map(prev)
            next.delete(chapterId)
            return new Map(next)
          })
        } else if (downloadEvent.event == 'ImageSuccess') {
          const { chapterId, current } = downloadEvent.data
          setProgresses((prev) => {
            const progressData = prev.get(chapterId)
            if (progressData === undefined) {
              return prev
            }
            const next = new Map(prev)
            const percentage = Math.round((current / progressData.total) * 100)
            next.set(chapterId, { ...progressData, current, percentage })
            return new Map(next)
          })
        } else if (downloadEvent.event == 'ImageError') {
          const { chapterId, errMsg } = downloadEvent.data
          const progressData = progressesRef.current.get(chapterId)
          if (progressData === undefined) {
            return
          }
          notificationRef.current.error({
            message: `${progressData.comicTitle} - ${progressData.chapterTitle}下载图片失败`,
            description: errMsg,
          })
        } else if (downloadEvent.event == 'Speed') {
          const { speed } = downloadEvent.data
          setDownloadSpeed(speed)
        }
      })
      .then((unListenFn) => {
        if (mounted) {
          unListen = unListenFn
        } else {
          unListenFn()
        }
      })

    return () => {
      mounted = false
      unListen?.()
    }
  }, [])

  // 通过对话框选择下载目录
  async function selectDownloadDir() {
    const selectedDirPath = await open({ directory: true })
    if (selectedDirPath === null) {
      return
    }
    setConfig((prev) => {
      if (prev === undefined) {
        return prev
      }
      return { ...prev, downloadDir: selectedDirPath }
    })
  }

  async function revealDownloadDir() {
    try {
      await revealItemInDir(config.downloadDir)
    } catch (error) {
      if (typeof error === 'string') {
        notification.error({
          message: '打开下载目录失败',
          description: `打开下载目录"${config.downloadDir}"失败: ${error}`,
          duration: 0,
        })
      } else {
        notification.error({
          message: '打开下载目录失败',
          description: `打开下载目录"${config.downloadDir}"失败，请联系开发者`,
          duration: 0,
        })
        console.error(error)
      }
    }
  }

  return (
    <div className={`h-full flex flex-col ${className}`}>
      <span className="h-38px text-lg font-bold">下载列表</span>
      <div className="flex gap-col-1">
        <Input value={config.downloadDir} prefix="下载目录" size="small" readOnly onClick={selectDownloadDir} />
        <Button size="small" onClick={revealDownloadDir}>
          打开目录
        </Button>
      </div>
      <span>下载速度: {downloadSpeed}</span>
      <div className="overflow-auto">
        {sortedProgresses.map(([chapterId, { comicTitle, chapterTitle, percentage, current, total, retryAfter }]) => (
          <div className="grid grid-cols-[1fr_1fr_2fr]" key={chapterId}>
            <span className="mb-1! text-ellipsis whitespace-nowrap overflow-hidden" title={comicTitle}>
              {comicTitle}
            </span>
            <span className="mb-1! text-ellipsis whitespace-nowrap overflow-hidden" title={chapterTitle}>
              {chapterTitle}
            </span>
            <DownloadingProgress retryAfter={retryAfter} total={total} percentage={percentage} current={current} />
          </div>
        ))}
      </div>
    </div>
  )
}

interface DownloadingProgressProps {
  retryAfter: number
  total: number
  percentage: number
  current: number
}

function DownloadingProgress({ retryAfter, total, percentage, current }: DownloadingProgressProps) {
  if (retryAfter !== 0) {
    return (
      <span className="mb-1! text-ellipsis whitespace-nowrap overflow-hidden">
        风控中，将在{retryAfter}秒后自动重试
      </span>
    )
  } else if (total === 0) {
    return <span className="mb-1! text-ellipsis whitespace-nowrap overflow-hidden">等待中</span>
  }

  return <Progress percent={percentage} format={() => `${current}/${total}`} />
}

export default DownloadingPane
