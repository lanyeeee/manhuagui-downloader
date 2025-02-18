import { App as AntdApp, Button, Input, Progress } from 'antd'
import { commands, Config, DownloadTaskEvent, DownloadTaskState, events } from '../bindings.ts'
import { useEffect, useMemo, useRef, useState } from 'react'
import { open } from '@tauri-apps/plugin-dialog'
import SettingsDialog from '../components/SettingsDialog.tsx'

type ProgressData = DownloadTaskEvent & { percentage: number; indicator: string }

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
        return b[1].totalImgCount - a[1].totalImgCount
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
    let unListenDownloadEvent: () => void | undefined
    let unListenDownloadTaskEvent: () => void | undefined

    events.downloadEvent
      .listen(({ payload: downloadEvent }) => {
        if (downloadEvent.event === 'Sleeping') {
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
        } else if (downloadEvent.event == 'Speed') {
          const { speed } = downloadEvent.data
          setDownloadSpeed(speed)
        }
      })
      .then((unListenFn) => {
        if (mounted) {
          unListenDownloadEvent = unListenFn
        } else {
          unListenFn()
        }
      })

    events.downloadTaskEvent
      .listen(({ payload: downloadTaskEvent }) => {
        setProgresses((prev) => {
          const { state, chapterInfo, downloadedImgCount, totalImgCount } = downloadTaskEvent

          const percentage = (downloadedImgCount / totalImgCount) * 100

          let indicator = ''
          if (state === 'Pending') {
            indicator = `等待中 ${downloadedImgCount}/${totalImgCount}`
          } else if (state === 'Downloading') {
            indicator = `下载中 ${downloadedImgCount}/${totalImgCount}`
          } else if (state === 'Paused') {
            indicator = `已暂停 ${downloadedImgCount}/${totalImgCount}`
          } else if (state === 'Cancelled') {
            indicator = `已取消 ${downloadedImgCount}/${totalImgCount}`
          } else if (state === 'Completed') {
            indicator = `下载完成 ${downloadedImgCount}/${totalImgCount}`
          } else if (state === 'Failed') {
            indicator = `下载失败 ${downloadedImgCount}/${totalImgCount}`
          }

          const next = new Map(prev)
          next.set(chapterInfo.chapterId, { ...downloadTaskEvent, percentage, indicator })
          return new Map(next)
        })
      })
      .then((unListenFn) => {
        if (mounted) {
          unListenDownloadTaskEvent = unListenFn
        } else {
          unListenFn()
        }
      })

    return () => {
      mounted = false
      unListenDownloadEvent?.()
      unListenDownloadTaskEvent?.()
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
        {sortedProgresses.map(([chapterId, { state, chapterInfo, percentage, indicator }]) => (
          <div className="grid grid-cols-[1fr_1fr_2fr]" key={chapterId}>
            <span className="mb-1! text-ellipsis whitespace-nowrap overflow-hidden" title={chapterInfo.comicTitle}>
              {chapterInfo.comicTitle}
            </span>
            <span className="mb-1! text-ellipsis whitespace-nowrap overflow-hidden" title={chapterInfo.chapterTitle}>
              {chapterInfo.chapterTitle}
            </span>
            <Progress status={stateToStatus(state)} percent={percentage} format={() => indicator} />
          </div>
        ))}
      </div>
    </div>
  )
}

export default DownloadingPane
