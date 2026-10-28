import { useState } from 'react'
import type { CardSummary } from '../api'
import { gridSrcSet, scryfallImage, type Face } from './images'

/** Matches the grid column width, so the browser picks `grid` on most screens. */
const SIZES = '(max-width: 40rem) 45vw, 220px'

export default function CardTile({ card }: { card: CardSummary }) {
  const [face, setFace] = useState<Face>('front')
  const [loaded, setLoaded] = useState(false)

  if (!card.image_small) {
    return <span className="no-image">{card.name}</span>
  }

  return (
    <div className="tile" data-loaded={loaded}>
      <img className="placeholder" src={scryfallImage(card.image_small, 'thumb', face)} alt="" aria-hidden loading="lazy" />
      <img
        key={face}
        className="full"
        srcSet={gridSrcSet(card.image_small, face)}
        sizes={SIZES}
        src={scryfallImage(card.image_small, 'grid', face)}
        alt={card.name}
        loading="lazy"
        width={488}
        height={680}
        onLoad={() => setLoaded(true)}
      />
      {card.double_faced && (
        <button
          type="button"
          className="flip"
          aria-label={`Show the ${face === 'front' ? 'back' : 'front'} face of ${card.name}`}
          aria-pressed={face === 'back'}
          onClick={(event) => {
            event.stopPropagation()
            setLoaded(false)
            setFace(face === 'front' ? 'back' : 'front')
          }}
        >
          ⟲
        </button>
      )}
    </div>
  )
}
