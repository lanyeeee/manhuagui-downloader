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
    <div className="h-full">
      {completedProgresses.map(([chapterId, { chapterInfo }]) => (
        <div className="grid grid-cols-[1fr_1fr]" key={chapterId}>
          <span className="mb-1! text-ellipsis whitespace-nowrap overflow-hidden" title={chapterInfo.comicTitle}>
            {chapterInfo.comicTitle}
          </span>
          <span className="mb-1! text-ellipsis whitespace-nowrap overflow-hidden" title={chapterInfo.chapterTitle}>
            {chapterInfo.chapterTitle}
          </span>
        </div>
      ))}
    </div>
  )
}

export default CompletedProgresses
