import { Comic, commands, SearchResult } from '../bindings.ts'
import { CurrentTabName } from '../types.ts'
import { useState } from 'react'
import { App as AntdApp, Button, Pagination, Space } from 'antd'
import ComicCard from '../components/ComicCard.tsx'
import { FloatLabelInput } from '../components/FloatLabelInput.tsx'
import { ArrowRightIcon, MagnifyingGlassIcon } from '@phosphor-icons/react'

interface Props {
  setPickedComic: (comic: Comic | undefined) => void
  setCurrentTabName: (currentTabName: CurrentTabName) => void
}

function SearchPane({ setPickedComic, setCurrentTabName }: Props) {
  const { message } = AntdApp.useApp()

  const [searchInput, setSearchInput] = useState<string>('')
  const [comicIdInput, setComicIdInput] = useState<string>('')
  const [searchPageNum, setSearchPageNum] = useState<number>(1)
  const [searchResult, setSearchResult] = useState<SearchResult>()
  const [searching, setSearching] = useState<boolean>(false)
  const [picking, setPicking] = useState<boolean>(false)

  async function search(keyword: string, pageNum: number) {
    setSearchPageNum(pageNum)
    setSearching(true)

    const result = await commands.search(keyword, pageNum)
    if (result.status === 'error') {
      console.error(result.error)
      return
    }

    setSearchResult(result.data)
    setSearching(false)
  }

  function getComicIdFromComicIdInput(): number | undefined {
    const comicIdString = comicIdInput.trim()
    const comicId = parseInt(comicIdString)
    if (!isNaN(comicId)) {
      return comicId
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
      message.error('漫画ID格式错误，请输入正确的漫画ID或漫画链接')
      return
    }

    setPicking(true)

    const result = await commands.getComic(comicId)
    if (result.status === 'error') {
      console.error(result.error)
      return
    }

    setPicking(false)

    setPickedComic(result.data)
    setCurrentTabName('chapter')
  }

  return (
    <div className="h-full flex flex-col">
      <Space.Compact className="box-border px-2 pt-2">
        <FloatLabelInput
          label="关键词"
          allowClear
          value={searchInput}
          onChange={(e) => setSearchInput(e.target.value)}
          onKeyDown={async (e) => {
            if (e.key === 'Enter') await search(searchInput.trim(), 1)
          }}
        />
        <Button
          loading={searching}
          type="primary"
          className="w-15%!"
          icon={<MagnifyingGlassIcon size={22} className="mt-2px" />}
          onClick={() => search(searchInput.trim(), 1)}
        />
      </Space.Compact>

      <Space.Compact className="box-border px-2 pt-1.5">
        <FloatLabelInput
          label="漫画ID (链接也行)"
          allowClear
          value={comicIdInput}
          onChange={(e) => setComicIdInput(e.target.value)}
          onKeyDown={async (e) => {
            if (e.key === 'Enter') await pickComic()
          }}
        />
        <Button
          loading={picking}
          type="primary"
          className="w-15%!"
          icon={<ArrowRightIcon size={22} className="mt-2px" />}
          onClick={() => pickComic()}
        />
      </Space.Compact>

      {searchResult && (
        <>
          <div className="flex flex-col overflow-auto">
            <div className="flex flex-col gap-row-2 overflow-auto p-2">
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
          </div>

          <Pagination
            className="p-2 mt-auto"
            current={searchPageNum}
            pageSize={10}
            total={searchResult.total}
            showSizeChanger={false}
            simple
            onChange={(pageNum) => search(searchInput.trim(), pageNum)}
          />
        </>
      )}
    </div>
  )
}

export default SearchPane
