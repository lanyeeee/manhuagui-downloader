import { App as AntdApp, Button, Input, Progress } from 'antd'
import { commands, Config, events } from '../bindings.ts'
import { useEffect, useMemo, useRef, useState } from 'react'
import { open } from '@tauri-apps/plugin-dialog'
import SettingsDialog from '../components/SettingsDialog.tsx'

type ProgressData = {
  comicTitle: string
  chapterTitle: string
  current: number
  total: number
  percentage: number
  indicator: string
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
  const [settingsDialogShowing, setSettingsDialogShowing] = useState<boolean>(false)
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
          const { chapterId, comicTitle, chapterTitle } = downloadEvent.data
          const progressData: ProgressData = {
            comicTitle,
            chapterTitle,
            current: 0,
            total: 0,
            percentage: 0,
            indicator: '等待中',
          }
          setProgresses((prev) => new Map(prev).set(chapterId, progressData))
        } else if (downloadEvent.event === 'ChapterSleeping') {
          const { chapterId, remainingSec } = downloadEvent.data
          setProgresses((prev) => {
            const progressData = prev.get(chapterId)
            if (progressData === undefined) {
              return prev
            }
            const next = new Map(prev)
            next.set(chapterId, { ...progressData, indicator: `将在${remainingSec}秒后继续下载` })
            return new Map(next)
          })
        } else if (downloadEvent.event == 'ChapterEnd') {
          const { chapterId } = downloadEvent.data
          setProgresses((prev) => {
            const progressData = prev.get(chapterId)
            if (progressData === undefined) {
              return prev
            }
            const next = new Map(prev)
            next.delete(chapterId)
            return new Map(next)
          })
        } else if (downloadEvent.event == 'ImageSuccess') {
          const { chapterId, current, total } = downloadEvent.data
          setProgresses((prev) => {
            const progressData = prev.get(chapterId)
            if (progressData === undefined) {
              return prev
            }
            const next = new Map(prev)
            const percentage = Math.round((current / total) * 100)
            next.set(chapterId, { ...progressData, current, total, percentage, indicator: `${current}/${total}` })
            return new Map(next)
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

  async function showDownloadDirInFileManager() {
    const result = await commands.showPathInFileManager(config.downloadDir)
    if (result.status === 'error') {
      console.error(result.error)
    }
  }

  return (
    <div className={`h-full flex flex-col ${className}`}>
      <span className="h-38px text-lg font-bold">下载列表</span>
      <div className="flex gap-col-1">
        <Input value={config.downloadDir} prefix="下载目录" size="small" readOnly onClick={selectDownloadDir} />
        <Button size="small" onClick={showDownloadDirInFileManager}>
          打开目录
        </Button>
        <Button size="small" onClick={() => setSettingsDialogShowing(true)}>
          更多设置
        </Button>
        <SettingsDialog
          settingsDialogShowing={settingsDialogShowing}
          setSettingsDialogShowing={setSettingsDialogShowing}
          config={config}
          setConfig={setConfig}
        />
      </div>
      <span>下载速度: {downloadSpeed}</span>
      <div className="overflow-auto">
        {sortedProgresses.map(([chapterId, { comicTitle, chapterTitle, percentage, total, indicator }]) => (
          <div className="grid grid-cols-[1fr_1fr_2fr]" key={chapterId}>
            <span className="mb-1! text-ellipsis whitespace-nowrap overflow-hidden" title={comicTitle}>
              {comicTitle}
            </span>
            <span className="mb-1! text-ellipsis whitespace-nowrap overflow-hidden" title={chapterTitle}>
              {chapterTitle}
            </span>
            <DownloadingProgress total={total} percentage={percentage} indicator={indicator} />
          </div>
        ))}
      </div>
    </div>
  )
}

interface DownloadingProgressProps {
  total: number
  percentage: number
  indicator: string
}

function DownloadingProgress({ total, percentage, indicator }: DownloadingProgressProps) {
  if (total === 0) {
    return <span className="mb-1! text-ellipsis whitespace-nowrap overflow-hidden">等待中</span>
  } else {
    return <Progress percent={percentage} format={() => indicator} />
  }
}

export default DownloadingPane
