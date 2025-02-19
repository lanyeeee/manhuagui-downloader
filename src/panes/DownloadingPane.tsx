import { Button, Input, Tabs, TabsProps } from 'antd'
import { commands, Config, events } from '../bindings.ts'
import { useEffect, useMemo, useState } from 'react'
import { open } from '@tauri-apps/plugin-dialog'
import SettingsDialog from '../components/SettingsDialog.tsx'
import { ProgressData } from '../types.ts'
import UncompletedProgresses from '../components/UncompletedProgresses.tsx'
import CompletedProgresses from '../components/CompletedProgresses.tsx'

interface Props {
  className?: string
  config: Config
  setConfig: (value: Config | undefined | ((prev: Config | undefined) => Config | undefined)) => void
}

function DownloadingPane({ className, config, setConfig }: Props) {
  const [progresses, setProgresses] = useState<Map<number, ProgressData>>(new Map())
  const [downloadSpeed, setDownloadSpeed] = useState<string>()
  const [settingsDialogShowing, setSettingsDialogShowing] = useState<boolean>(false)

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
            indicator = `排队中`
          } else if (state === 'Downloading') {
            indicator = `下载中`
          } else if (state === 'Paused') {
            indicator = `已暂停`
          } else if (state === 'Cancelled') {
            indicator = `已取消`
          } else if (state === 'Completed') {
            indicator = `下载完成`
          } else if (state === 'Failed') {
            indicator = `下载失败`
          }
          if (totalImgCount !== 0) {
            indicator += ` ${downloadedImgCount}/${totalImgCount}`
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

  const tabItems = useMemo<TabsProps['items']>(
    () => [
      { key: 'uncompleted', label: '未完成', children: <UncompletedProgresses progresses={progresses} /> },
      { key: 'completed', label: '已完成', children: <CompletedProgresses progresses={progresses} /> },
    ],
    [progresses],
  )

  return (
    <div className={`h-full flex flex-col ${className}`}>
      <span className="h-11 text-lg font-bold">下载列表</span>
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
      <Tabs size="small" className="overflow-auto h-full" items={tabItems} />
    </div>
  )
}

export default DownloadingPane
