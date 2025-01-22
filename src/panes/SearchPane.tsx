import { Comic, commands, SearchResult } from '../bindings.ts'
import { CurrentTabName } from '../types.ts'
import { useState } from 'react'
import { App as AntdApp, Button, Input, Pagination } from 'antd'
import ComicCard from '../components/ComicCard.tsx'
import isNumeric from 'antd/es/_util/isNumeric'

interface Props {
  setPickedComic: (comic: Comic | undefined) => void
  setCurrentTabName: (currentTabName: CurrentTabName) => void
}

function SearchPane({ setPickedComic, setCurrentTabName }: Props) {
  const { notification } = AntdApp.useApp()

  const [searchInput, setSearchInput] = useState<string>('')
  const [comicIdInput, setComicIdInput] = useState<string>('')
  const [searchPageNum, setSearchPageNum] = useState<number>(1)
  const [searchResult, setSearchResult] = useState<SearchResult>()

  async function search(keyword: string, pageNum: number) {
    console.log(keyword, pageNum)
    setSearchPageNum(pageNum)
    const result = await commands.search(keyword, pageNum)
    if (result.status === 'error') {
      notification.error({
        message: '搜索失败',
        description: result.error,
        duration: 0,
      })
      return
    }
    setSearchResult(result.data)
    console.log(result.data)
  }

  function getComicIdFromComicIdInput(): number | undefined {
    const comicIdString = comicIdInput.trim()
    if (isNumeric(comicIdString)) {
      return parseInt(comicIdString)
    }

    const regex = /\/comic\/(\d+)/
    const match = comicIdString.match(regex)
    if (match === null || match[1] === null) {
      return
    }
    return parseInt(match[1])
  }

  async function pickComic() {
    const comicId = getComicIdFromComicIdInput()

    if (comicId === undefined) {
      notification.error({
        message: '漫画ID格式错误',
        description: '请输入漫画ID或漫画链接',
        duration: 0,
      })
      return
    }

    const result = await commands.getComic(comicId)
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
    <div className="h-full flex flex-col">
      <div className="flex flex-col">
        <div className="flex">
          <Input
            prefix="关键词:"
            size="small"
            allowClear
            value={searchInput}
            onChange={(e) => setSearchInput(e.target.value)}
            onKeyDown={async (e) => {
              if (e.key === 'Enter') await search(searchInput.trim(), 1)
            }}
          />
          <Button size="small" onClick={() => search(searchInput.trim(), 1)}>
            搜索
          </Button>
        </div>
        <div className="flex">
          <Input
            prefix="漫画ID:"
            size="small"
            placeholder="链接也行"
            allowClear
            value={comicIdInput}
            onChange={(e) => setComicIdInput(e.target.value)}
            onKeyDown={async (e) => {
              if (e.key === 'Enter') await pickComic()
            }}
          />
          <Button size="small" onClick={() => pickComic()}>
            直达
          </Button>
        </div>
      </div>

      {searchResult && (
        <div className="h-full flex flex-col gap-row-1 overflow-auto">
          <div className="h-full flex flex-col gap-row-2 overflow-auto p-2">
            {searchResult.comics.map((comic) => (
              <ComicCard
                key={comic.id}
                comicId={comic.id}
                comicTitle={comic.title}
                comicCover={comic.cover}
                comicSubtitle={comic.subtitle}
                comicAuthors={comic.authors}
                comicGenres={comic.genres}
                comicLastUpdateTime={comic.updateTime}
                setPickedComic={setPickedComic}
                setCurrentTabName={setCurrentTabName}
              />
            ))}
          </div>
          <Pagination
            current={searchPageNum}
            pageSize={10}
            total={searchResult.total}
            showSizeChanger={false}
            simple
            onChange={(pageNum) => search(searchInput.trim(), pageNum)}
          />
        </div>
      )}
    </div>
  )
}

export default SearchPane
