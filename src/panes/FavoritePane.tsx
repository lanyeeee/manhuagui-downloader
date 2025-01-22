import { Comic, commands, GetFavoriteResult, UserProfile } from '../bindings.ts'
import { CurrentTabName } from '../types.ts'
import { App as AntdApp, Pagination } from 'antd'
import { useCallback, useEffect, useState } from 'react'
import ComicCard from '../components/ComicCard.tsx'

interface Props {
  userProfile: UserProfile | undefined
  setPickedComic: (comic: Comic | undefined) => void
  setCurrentTabName: (currentTabName: CurrentTabName) => void
}

function FavoritePane({ userProfile, setPickedComic, setCurrentTabName }: Props) {
  const { notification } = AntdApp.useApp()
  const [favoritePageNum, setFavoritePageNum] = useState(1)
  const [getFavoriteResult, setGetFavoriteResult] = useState<GetFavoriteResult>()

  const getFavourite = useCallback(
    async (pageNum: number) => {
      setFavoritePageNum(pageNum)
      const result = await commands.getFavorite(pageNum)
      if (result.status === 'error') {
        notification.error({ message: '获取收藏失败', description: result.error, duration: 0 })
        return
      }
      console.log('getFavourite')
      setGetFavoriteResult(result.data)
      console.log(result.data)
    },
    [notification],
  )

  useEffect(() => {
    getFavourite(1).then()
  }, [userProfile, getFavourite])

  return (
    <div className="h-full flex flex-col">
      {getFavoriteResult && (
        <div className="h-full flex flex-col gap-row-1 overflow-auto">
          <div className="h-full flex flex-col gap-row-2 overflow-auto p-2">
            {getFavoriteResult.comics.map((comic) => (
              <ComicCard
                key={comic.id}
                comicId={comic.id}
                comicTitle={comic.title}
                comicCover={comic.cover}
                comicLastUpdateTime={comic.lastUpdate}
                comicLastReadTime={comic.lastRead}
                setPickedComic={setPickedComic}
                setCurrentTabName={setCurrentTabName}
              />
            ))}
          </div>
          <Pagination
            current={favoritePageNum}
            pageSize={20}
            total={getFavoriteResult.total}
            showSizeChanger={false}
            simple
            onChange={(pageNum) => getFavourite(pageNum)}
          />
        </div>
      )}
    </div>
  )
}

export default FavoritePane
