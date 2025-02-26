import { ProgressData } from '../types.ts'
import { useMemo } from 'react'

interface Props {
  progresses: Map<number, ProgressData>
}

function CompletedProgresses({ progresses }: Props) {
  const completedProgresses = useMemo<[number, ProgressData][]>(
    () =>
      Array.from(progresses.entries())
        .filter(([, { state }]) => state === 'Completed')
        .sort((a, b) => {
          return b[1].totalImgCount - a[1].totalImgCount
        }),
    [progresses],
  )

  return (
    <div className="h-full flex flex-col gap-row-2 px-1">
      {completedProgresses.map(([chapterId, { chapterInfo }]) => (
        <div className="grid grid-cols-[1fr_1fr_1fr] py-2 px-4 bg-gray-100 rounded-lg" key={chapterId}>
          <span className="text-ellipsis whitespace-nowrap overflow-hidden" title={chapterInfo.comicTitle}>
            {chapterInfo.comicTitle}
          </span>
          <span className="text-ellipsis whitespace-nowrap overflow-hidden" title={chapterInfo.groupName}>
            {chapterInfo.groupName}
          </span>
          <span className="text-ellipsis whitespace-nowrap overflow-hidden" title={chapterInfo.chapterTitle}>
            {chapterInfo.chapterTitle}
          </span>
        </div>
      ))}
    </div>
  )
}

export default CompletedProgresses
