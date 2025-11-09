import { AuthorRespData, DownloadTaskEvent } from './bindings.ts'

export interface ComicInfo {
  name: string
  path_word: string
  cover: string
  author: AuthorRespData[]
}

export type CurrentTabName = 'search' | 'favorite' | 'downloaded' | 'chapter'

export type ProgressData = Extract<DownloadTaskEvent, { event: 'Create' }>['data'] & {
  percentage: number
  indicator: string
  retryAfter: number
}
