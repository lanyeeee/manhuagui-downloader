import { Comic, commands } from '../bindings.ts'
import { CurrentTabName } from '../types.ts'
import { useEffect, useMemo, useRef, useState } from 'react'
import { App as AntdApp, Pagination } from 'antd'
import DownloadedComicCard from '../components/DownloadedComicCard.tsx'
import { MessageInstance } from 'antd/es/message/interface'

interface Props {
  setPickedComic: (value: Comic | undefined) => void
  currentTabName: CurrentTabName
  setCurrentTabName: (currentTabName: CurrentTabName) => void
}

function DownloadedPane({ setPickedComic, currentTabName, setCurrentTabName }: Props) {
  const { message, notification } = AntdApp.useApp()

  const [downloadedComics, setDownloadedComics] = useState<Comic[]>([])
  const [downloadedPageNum, setDownloadedPageNum] = useState<number>(1)

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

  return (
    <div className="h-full flex flex-col overflow-auto">
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
