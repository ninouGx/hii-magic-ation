import { SORT_FIELDS, type SortKey } from './urlState'

type Props = { sort: SortKey[]; onChange: (sort: SortKey[]) => void }

export default function SortControls({ sort, onChange }: Props) {
  const update = (index: number, key: SortKey) => onChange(sort.map((current, i) => (i === index ? key : current)))
  const unused = SORT_FIELDS.find(([field]) => !sort.some((key) => key.field === field))

  return (
    <div className="sort">
      <span>Sort by</span>
      {sort.map((key, index) => (
        <span key={index} className="sort-key">
          {index > 0 && <span>then</span>}
          <select
            aria-label={`Sort key ${index + 1}`}
            value={key.field}
            onChange={(event) => update(index, { ...key, field: event.target.value })}
          >
            {SORT_FIELDS.map(([field, label]) => (
              <option key={field} value={field}>
                {label}
              </option>
            ))}
          </select>
          <button
            type="button"
            aria-label={`Direction of sort key ${index + 1}`}
            onClick={() => update(index, { ...key, direction: key.direction === 'asc' ? 'desc' : 'asc' })}
          >
            {key.direction === 'asc' ? '↑ asc' : '↓ desc'}
          </button>
          {sort.length > 1 && (
            <button
              type="button"
              aria-label={`Remove sort key ${index + 1}`}
              onClick={() => onChange(sort.filter((_, i) => i !== index))}
            >
              ×
            </button>
          )}
        </span>
      ))}
      {unused && (
        <button type="button" onClick={() => onChange([...sort, { field: unused[0], direction: 'asc' }])}>
          + sort key
        </button>
      )}
    </div>
  )
}
