import { App as AntdApp, Modal, Input, Button, Select, Checkbox } from 'antd'
import { useEffect, useState, useMemo, useRef } from 'react'
import { commands, Config, events, LogEvent, LogLevel } from '../bindings.ts'
import { path } from '@tauri-apps/api'
import { appDataDir } from '@tauri-apps/api/path'

interface Props {
  logViewerShowing: boolean
  setLogViewerShowing: (showing: boolean) => void
  config: Config
  setConfig: (value: Config | undefined | ((prev: Config | undefined) => Config | undefined)) => void
}

type LogRecord = LogEvent & { id: number; formatedLog: string }

function LogViewer({ logViewerShowing, setLogViewerShowing, config, setConfig }: Props) {
  const { notification } = AntdApp.useApp()
  const [logRecords, setLogRecords] = useState<LogRecord[]>([])
  const [searchText, setSearchText] = useState('')
  const [selectedLevel, setSelectedLevel] = useState<LogLevel>('INFO')
  const [logsDirSize, setLogsDirSize] = useState<number>(0)
  const nextLogRecordId = useRef<number>(1)

  useEffect(() => {
    let mounted = true
    let unListenLogEvent: () => void | undefined

    events.logEvent
      .listen(async ({ payload: logEvent }) => {
        setLogRecords((prev) => [
          ...prev,
          {
            ...logEvent,
            id: nextLogRecordId.current++,
            formatedLog: formatLogEvent(logEvent),
          },
        ])
        const { level, fields } = logEvent
        if (level === 'ERROR') {
          notification.error({
            message: fields['err_title'] as string,
            description: fields['message'] as string,
            duration: 0,
          })
        }
      })
      .then((unListenFn) => {
        if (mounted) {
          unListenLogEvent = unListenFn
        } else {
          unListenFn()
        }
      })

    return () => {
      mounted = false
      unListenLogEvent?.()
    }
  }, [notification])

  useEffect(() => {
    if (!logViewerShowing) {
      return
    }
    commands.getLogsDirSize().then((result) => {
      if (result.status === 'error') {
        console.error(result.error)
        return
      }

      setLogsDirSize(result.data)
    })
  }, [logViewerShowing])

  const formatedLogsDirSize = useMemo(() => {
    const units = ['B', 'KB', 'MB']
    let size = logsDirSize
    let unitIndex = 0

    while (size >= 1024 && unitIndex < 2) {
      size /= 1024
      unitIndex++
    }

    // 保留两位小数
    return `${size.toFixed(2)} ${units[unitIndex]}`
  }, [logsDirSize])

  const filteredLogs = useMemo(() => {
    return logRecords.filter(({ level, formatedLog }) => {
      // 定义日志等级的优先级顺序
      const logLevelPriority = {
        TRACE: 0,
        DEBUG: 1,
        INFO: 2,
        WARN: 3,
        ERROR: 4,
      }
      // 首先按日志等级筛选
      if (logLevelPriority[level] < logLevelPriority[selectedLevel]) {
        return false
      }
      // 然后按搜索文本筛选
      if (searchText === '') {
        return true
      }

      return formatedLog.toLowerCase().includes(searchText.toLowerCase())
    })
  }, [logRecords, searchText, selectedLevel])

  function getLevelStyles(level: LogLevel) {
    switch (level) {
      case 'TRACE':
        return 'text-gray-400'
      case 'DEBUG':
        return 'text-green-400'
      case 'INFO':
        return 'text-blue-400'
      case 'WARN':
        return 'text-yellow-400'
      case 'ERROR':
        return 'text-red-400'
    }
  }

  const logLevelOptions = [
    { value: 'TRACE', label: 'TRACE' },
    { value: 'DEBUG', label: 'DEBUG' },
    { value: 'INFO', label: 'INFO' },
    { value: 'WARN', label: 'WARN' },
    { value: 'ERROR', label: 'ERROR' },
  ]

  function formatLogEvent(logEvent: LogEvent): string {
    const { timestamp, level, fields, target, filename, line_number } = logEvent
    const fields_str = Object.entries(fields)
      .map(([key, value]) => `${key}=${value}`)
      .join(' ')
    return `${timestamp} ${level} ${target}: ${filename}:${line_number} ${fields_str}`
  }

  function clearLogRecords() {
    setLogRecords([])
    nextLogRecordId.current = 1
  }

  async function showLogsDirInFileManager() {
    const logsDir = await path.join(await appDataDir(), '日志')
    const result = await commands.showPathInFileManager(logsDir)
    if (result.status === 'error') {
      console.error(result.error)
    }
  }

  return (
    <Modal
      title={<div className="flex items-center">日志目录总大小：{formatedLogsDirSize}</div>}
      open={logViewerShowing}
      onCancel={() => setLogViewerShowing(false)}
      width="95%"
      footer={null}>
      <div className="mb-2 flex flex-wrap gap-2">
        <Input
          size="small"
          placeholder="搜索日志..."
          value={searchText}
          onChange={(e) => setSearchText(e.target.value)}
          style={{ width: 300 }}
          allowClear
        />
        <Select
          size="small"
          value={selectedLevel}
          onChange={setSelectedLevel}
          options={logLevelOptions}
          style={{ width: 120 }}
        />
        <div className="flex flex-wrap gap-2 ml-auto">
          <Button size="small" onClick={showLogsDirInFileManager}>
            打开日志目录
          </Button>
          <Checkbox
            className="select-none"
            checked={config.enableFileLogger}
            onChange={() =>
              setConfig((prev) => {
                if (prev === undefined) {
                  return prev
                }
                return { ...prev, enableFileLogger: !prev.enableFileLogger }
              })
            }>
            输出文件日志
          </Checkbox>
        </div>
      </div>

      <div className="h-[calc(100vh-300px)] overflow-auto bg-gray-900 p-3">
        {filteredLogs.map(({ id, level, formatedLog }) => (
          <div key={id} className={`p-1 hover:bg-white/10 whitespace-pre-wrap ${getLevelStyles(level)}`}>
            {formatedLog}
          </div>
        ))}
      </div>
      <div className="pt-1 flex">
        <Button className="ml-auto" size="small" onClick={clearLogRecords} danger>
          清空日志浏览器
        </Button>
      </div>
    </Modal>
  )
}

export default LogViewer
