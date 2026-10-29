import { useQuery, useQueryClient } from '@tanstack/react-query'
import { useEffect, useRef, useState, type ReactNode } from 'react'
import { fetchCard, type CardDetail as Card, type CardFace } from '../api'
import { scryfallImage, type Face } from './images'
import { tagsForPrint } from './tags'

const PERIOD_LABELS: Record<string, string> = { week: 'Week', month: 'Month', year: '2 years' }

type Props = {
  oracleId: string
  previous: string | null
  next: string | null
  onNavigate: (oracleId: string) => void
  onClose: () => void
  /** Renders extra controls in the header, given the card name once it is loaded. */
  actions?: (name: string) => ReactNode
}

/** Escape and a click on the backdrop close it, the arrow keys move through the result. */
export default function CardDetail({ oracleId, previous, next, onNavigate, onClose, actions }: Props) {
  const dialog = useRef<HTMLDialogElement>(null)
  const queryClient = useQueryClient()
  const card = useQuery({ queryKey: ['card', oracleId], queryFn: () => fetchCard(oracleId), staleTime: Infinity })

  useEffect(() => {
    const element = dialog.current
    if (element && !element.open) element.showModal()
  }, [])

  useEffect(() => {
    for (const neighbor of [previous, next]) {
      if (neighbor) {
        void queryClient.prefetchQuery({
          queryKey: ['card', neighbor],
          queryFn: () => fetchCard(neighbor),
          staleTime: Infinity,
        })
      }
    }
  }, [previous, next, queryClient])

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'ArrowLeft' && previous) onNavigate(previous)
      if (event.key === 'ArrowRight' && next) onNavigate(next)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [previous, next, onNavigate])

  return (
    <dialog
      ref={dialog}
      className="card-detail"
      aria-label={card.data?.name ?? 'Card'}
      onCancel={(event) => {
        event.preventDefault()
        onClose()
      }}
      onClick={(event) => {
        if (event.target === event.currentTarget) onClose()
      }}
    >
      <div className="card-detail-body">
        <header>
          {actions && card.data && actions(card.data.name)}
          <button type="button" aria-label="Previous card" disabled={!previous} onClick={() => previous && onNavigate(previous)}>
            ←
          </button>
          <button type="button" aria-label="Next card" disabled={!next} onClick={() => next && onNavigate(next)}>
            →
          </button>
          <button type="button" aria-label="Close" onClick={onClose}>
            ×
          </button>
        </header>
        {card.isError && <p role="alert">{card.error.message}</p>}
        {card.data ? <CardContent key={card.data.oracle_id} card={card.data} /> : !card.isError && <p>Loading…</p>}
      </div>
    </dialog>
  )
}

function CardContent({ card }: { card: Card }) {
  const [face, setFace] = useState<Face>('front')
  const [printId, setPrintId] = useState(card.default_print_id)
  const print = card.prints.find((candidate) => candidate.print_id === printId) ?? card.prints[0]
  const faces: CardFace[] = card.faces.length > 0 ? card.faces : [card]

  return (
    <div className="card-detail-content">
      <figure className="card-detail-image">
        {print?.image_small ? (
          <img
            key={`${print.print_id}-${face}`}
            src={scryfallImage(print.image_small, 'display', face)}
            alt={card.name}
            width={672}
            height={936}
          />
        ) : (
          <span className="no-image">{card.name}</span>
        )}
        {card.double_faced && (
          <button type="button" aria-pressed={face === 'back'} onClick={() => setFace(face === 'front' ? 'back' : 'front')}>
            ⟲ Show the {face === 'front' ? 'back' : 'front'} face
          </button>
        )}
        {print && (
          <figcaption>
            {print.set_code.toUpperCase()} #{print.collector_number}
            {print.artist && `, illustrated by ${print.artist}`}
            {print.price_eur !== null && `, ${print.price_eur.toFixed(2)} €`}
          </figcaption>
        )}
      </figure>

      <div className="card-detail-text">
        <h2>{card.name}</h2>
        {faces.map((side, index) => (
          <section key={index} className="face">
            {card.faces.length > 0 && <h3>{side.name}</h3>}
            <p className="type-line">
              {side.type_line} {side.mana_cost && <span className="mana">{side.mana_cost}</span>}
            </p>
            {side.oracle_text && <p className="oracle">{side.oracle_text}</p>}
            <Stats face={side} />
          </section>
        ))}
        {card.keywords.length > 0 && <p>Keywords: {card.keywords.join(', ')}</p>}

        <h3>Popularity</h3>
        <dl className="facts">
          <dt>EDHREC rank</dt>
          <dd>{card.edhrec_rank ?? 'none'}</dd>
          {card.commander_ranks.map((rank) => (
            <div key={rank.period}>
              <dt>Commander, {PERIOD_LABELS[rank.period] ?? rank.period}</dt>
              <dd>
                #{rank.rank}
                {rank.num_decks !== null && `, ${rank.num_decks.toLocaleString()} decks`}
                {rank.trend_zscore !== null && `, trend ${rank.trend_zscore.toFixed(2)}`}
              </dd>
            </div>
          ))}
        </dl>

        <Legalities card={card} />
        <Tags tags={tagsForPrint(card.tags, print)} />

        <h3>Prints ({card.prints.length})</h3>
        <ul className="prints">
          {card.prints.map((candidate) => (
            <li key={candidate.print_id}>
              <button
                type="button"
                aria-pressed={candidate.print_id === print?.print_id}
                title={`${candidate.set_code.toUpperCase()} #${candidate.collector_number}, ${candidate.released_at ?? ''}`}
                onClick={() => setPrintId(candidate.print_id)}
              >
                {candidate.image_small ? (
                  <img src={scryfallImage(candidate.image_small, 'thumb')} alt={candidate.set_code} loading="lazy" />
                ) : (
                  candidate.set_code
                )}
              </button>
            </li>
          ))}
        </ul>

        {card.rulings.length > 0 && (
          <>
            <h3>Rulings</h3>
            <ul className="rulings">
              {card.rulings.map((ruling, index) => (
                <li key={index}>
                  <time>{ruling.published_at}</time> {ruling.comment}
                </li>
              ))}
            </ul>
          </>
        )}
      </div>
    </div>
  )
}

function Stats({ face }: { face: CardFace }) {
  if (face.power !== null && face.toughness !== null) {
    return <p className="stats">{`${face.power}/${face.toughness}`}</p>
  }
  if (face.loyalty !== null) return <p className="stats">Loyalty {face.loyalty}</p>
  return null
}

function Legalities({ card }: { card: Card }) {
  return (
    <>
      <h3>Legality</h3>
      <ul className="legalities">
        {card.legalities.map((legality) => (
          <li key={legality.format} data-status={legality.status}>
            {legality.format} <span>{legality.status.replace('_', ' ')}</span>
          </li>
        ))}
      </ul>
    </>
  )
}

function Tags({ tags }: { tags: Card['tags'] }) {
  const kinds = [...new Set(tags.map((tag) => tag.kind))]
  return (
    <>
      <h3>Tags ({tags.length})</h3>
      {kinds.map((kind) => (
        <div key={kind}>
          <h4>
            {kind === 'art' ? 'Art, this print' : kind === 'oracle' ? 'Oracle' : kind} ({tags.filter((tag) => tag.kind === kind).length})
          </h4>
          <ul className="tags">
            {tags
              .filter((tag) => tag.kind === kind)
              .map((tag) => (
                <li key={tag.tag_id} title={tag.source}>
                  {tag.label}
                </li>
              ))}
          </ul>
        </div>
      ))}
    </>
  )
}
