import { Comic } from '../bindings.ts'
import { CurrentTabName } from '../types.ts'
import { Card } from 'antd'
import { useMemo } from 'react'

interface GroupInfo {
  name: string
  downloaded: number
  total: number
}

interface Props {
  comic: Comic
  setPickedComic: (comic: Comic | undefined) => void
  setCurrentTabName: (currentTabName: CurrentTabName) => void
}

function DownloadedComicCard({ comic, setPickedComic, setCurrentTabName }: Props) {
  const groupInfos = useMemo(() => {
    const groups = comic.groups

    const infos = Object.values(groups).map((chapterInfos) => {
      const groupInfo: GroupInfo = {
        name: chapterInfos[0].groupName,
        downloaded: chapterInfos.filter((chapterInfo) => chapterInfo.isDownloaded).length,
        total: chapterInfos.length,
      }
      return groupInfo
    })

    infos.sort((a, b) => b.total - a.total)
    return infos
  }, [comic.groups])

  function pickComic() {
    setPickedComic(comic)
    setCurrentTabName('chapter')
  }

  return (
    <Card hoverable={true} className="cursor-auto m-0! rounded-none" styles={{ body: { padding: '0.25rem' } }}>
      <div className="flex">
        <img
          className="w-24 object-cover mr-4 cursor-pointer transition-transform duration-200 hover:scale-106"
          src={comic.cover}
          alt=""
          onClick={() => pickComic()}
        />
        <div className="flex flex-col h-full w-full">
          <span
            className="font-bold text-xl line-clamp-3 cursor-pointer transition-colors duration-200 hover:text-blue-5"
            onClick={() => pickComic()}>
            {comic.title}
            {comic.subtitle && `(${comic.subtitle})`}
          </span>
          {comic.authors !== undefined && <span className="text-red">作者：{comic.authors.join(', ')}</span>}
          {comic.genres !== undefined && <span className="text-black">类型：{comic.genres.join(' ')}</span>}
          {groupInfos.map((groupInfo) => (
            <span key={groupInfo.name} className="text-black">
              {groupInfo.name}：{groupInfo.downloaded}/{groupInfo.total}
            </span>
          ))}
        </div>
      </div>
    </Card>
  )
}

export default DownloadedComicCard
