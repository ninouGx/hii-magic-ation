import { describe, expect, it } from 'vitest'
import { gridSrcSet, scryfallImage } from './images'

const SMALL = 'https://cards.scryfall.io/small/front/0/0/001e9f20-5b15-41cb-bf82-46172decc235.jpg?1783906558'

describe('scryfallImage', () => {
  it('swaps the size folder and the extension', () => {
    expect(scryfallImage(SMALL, 'grid')).toBe(
      'https://cards.scryfall.io/grid/front/0/0/001e9f20-5b15-41cb-bf82-46172decc235.webp?1783906558',
    )
  })

  it('points to the back face', () => {
    expect(scryfallImage(SMALL, 'display', 'back')).toBe(
      'https://cards.scryfall.io/display/back/0/0/001e9f20-5b15-41cb-bf82-46172decc235.webp?1783906558',
    )
  })

  it('lists the three sizes with their widths', () => {
    const entries = gridSrcSet(SMALL, 'front').split(', ')

    expect(entries.map((entry) => entry.split(' ')[1])).toEqual(['146w', '488w', '672w'])
    expect(entries[0]).toContain('/thumb/front/')
  })
})
