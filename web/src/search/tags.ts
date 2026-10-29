import type { CardDetail } from '../api'

type Tag = CardDetail['tags'][number]

/** Art tags sit on an illustration, so the shown print decides which ones apply. */
export function tagsForPrint(tags: Tag[], print: { art_tag_ids: string[] } | undefined): Tag[] {
  if (!print) return tags
  const art = new Set(print.art_tag_ids)
  return tags.filter((tag) => tag.kind !== 'art' || art.has(tag.tag_id))
}
