import { Comic, commands } from '../bindings.ts'
import { CurrentTabName } from '../types.ts'
import { App as AntdApp, Card } from 'antd'

interface Props {
  comicId: number
  comicTitle: string
  comicCover: string
  comicSubtitle?: string | null
  comicAuthors?: string[]
  comicGenres?: string[]
  comicLastUpdateTime?: string
  comicLastReadTime?: string
  setPickedComic: (comic: Comic | undefined) => void
  setCurrentTabName: (currentTabName: CurrentTabName) => void
}

function ComicCard({
  comicId,
  comicTitle,
  comicCover,
  comicSubtitle,
  comicAuthors,
  comicGenres,
  comicLastUpdateTime,
  comicLastReadTime,
  setPickedComic,
  setCurrentTabName,
}: Props) {
  const { notification } = AntdApp.useApp()

  async function pickComic(id: number) {
    const result = await commands.getComic(id)
    if (result.status === 'error') {
      notification.error({
        message: '获取漫画信息失败',
        description: result.error,
        duration: 0,
      })
      return
    }
    console.log(result.data)
    setPickedComic(result.data)
    setCurrentTabName('chapter')
  }

  return (
    <Card hoverable={true} className="cursor-auto m-0! rounded-none" styles={{ body: { padding: '0.25rem' } }}>
      <div className="flex">
        <img
          className="w-24 object-cover mr-4 cursor-pointer transition-transform duration-200 hover:scale-106"
          src={comicCover}
          alt=""
          onClick={() => pickComic(comicId)}
        />
        <div className="flex flex-col h-full">
          <span
            className="font-bold text-xl line-clamp-3 cursor-pointer transition-colors duration-200 hover:text-blue-5"
            onClick={() => pickComic(comicId)}>
            {comicTitle}
            {comicSubtitle && `(${comicSubtitle})`}
          </span>
          {comicAuthors !== undefined && <span className="text-red">作者：{comicAuthors.join(', ')}</span>}
          {comicGenres !== undefined && <span className="text-black">类型：{comicGenres.join(' ')}</span>}
          {comicLastUpdateTime !== undefined && <span className="text-gray">上次更新：{comicLastUpdateTime}</span>}
          {comicLastReadTime !== undefined && <span className="text-gray">上次阅读：{comicLastReadTime}</span>}
        </div>
      </div>
    </Card>
  )
}

export default ComicCard
