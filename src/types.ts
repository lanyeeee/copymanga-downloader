import { DownloadEvent } from './bindings.ts'

export type CurrentTabName = 'search' | 'favorite' | 'downloaded' | 'chapter'

export type ProgressData = Extract<DownloadEvent, { event: 'TaskCreate' }>['data'] & {
  percentage: number
  indicator: string
  retryAfter: number
}
