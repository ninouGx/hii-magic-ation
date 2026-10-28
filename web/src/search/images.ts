export type Face = 'front' | 'back'

/** Scryfall WebP sizes with their pixel widths. */
export const IMAGE_WIDTHS = { thumb: 146, grid: 488, display: 672 } as const

export type ImageSize = keyof typeof IMAGE_WIDTHS

/** Every Scryfall size and face shares one URL apart from its folders and extension. */
export function scryfallImage(smallUrl: string, size: ImageSize, face: Face = 'front'): string {
  const url = new URL(smallUrl)
  url.pathname = url.pathname
    .replace(/^\/small\//, `/${size}/`)
    .replace(/\/front\//, `/${face}/`)
    .replace(/\.jpg$/, '.webp')
  return url.toString()
}

export function gridSrcSet(smallUrl: string, face: Face): string {
  return (Object.keys(IMAGE_WIDTHS) as ImageSize[])
    .map((size) => `${scryfallImage(smallUrl, size, face)} ${IMAGE_WIDTHS[size]}w`)
    .join(', ')
}
