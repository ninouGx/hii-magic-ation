import { useQuery } from '@tanstack/react-query'
import { useId, useState } from 'react'
import { ValueEditor, type ValueEditorProps } from 'react-querybuilder'
import { fetchFormats, fetchKeywords, fetchTags } from '../api'
import { COLORS, FIELD_KINDS, LEGALITY_STATUSES, type LegalityValue, type TagValue } from './fields'

export default function FieldValueEditor(props: ValueEditorProps) {
  switch (FIELD_KINDS[props.field]) {
    case 'legality':
      return <LegalityEditor {...props} />
    case 'colors':
      return <ColorEditor {...props} />
    case 'tag':
      return <TagEditor {...props} />
    case 'keywords':
      return <KeywordEditor {...props} />
    case 'bool':
      return (
        <input
          type="checkbox"
          aria-label={props.fieldData.label}
          checked={props.value === true}
          onChange={(event) => props.handleOnChange(event.target.checked)}
        />
      )
    case 'number':
      return props.operator === 'between' ? <BetweenEditor {...props} /> : <TextInput {...props} inputMode="decimal" />
    case 'pick':
    case 'year':
      return <TextInput {...props} placeholder={props.operator === 'in' ? 'comma separated' : undefined} />
    case 'text':
      return <TextInput {...props} placeholder={props.operator === 'regex' ? 'RE2 regex, no lookahead' : undefined} />
    default:
      return <ValueEditor {...props} />
  }
}

function TextInput(props: ValueEditorProps & { inputMode?: 'decimal'; placeholder?: string }) {
  return (
    <input
      className="rule-value"
      value={typeof props.value === 'string' || typeof props.value === 'number' ? props.value : ''}
      inputMode={props.inputMode}
      placeholder={props.placeholder}
      aria-label={props.fieldData.label}
      onChange={(event) => props.handleOnChange(event.target.value)}
    />
  )
}

function BetweenEditor({ value, handleOnChange, fieldData }: ValueEditorProps) {
  const [low = '', high = ''] = Array.isArray(value) ? value : []
  return (
    <span className="rule-value">
      <input
        inputMode="decimal"
        aria-label={`${fieldData.label} from`}
        value={low}
        onChange={(event) => handleOnChange([event.target.value, high])}
      />
      and
      <input
        inputMode="decimal"
        aria-label={`${fieldData.label} to`}
        value={high}
        onChange={(event) => handleOnChange([low, event.target.value])}
      />
    </span>
  )
}

function LegalityEditor({ value, handleOnChange }: ValueEditorProps) {
  const legality = value as LegalityValue
  const formats = useQuery({ queryKey: ['formats'], queryFn: fetchFormats, staleTime: Infinity })
  return (
    <span className="rule-value">
      <select
        aria-label="Format"
        value={legality.format}
        onChange={(event) => handleOnChange({ ...legality, format: event.target.value })}
      >
        {(formats.data ?? [legality.format]).map((format) => (
          <option key={format}>{format}</option>
        ))}
      </select>
      <select
        aria-label="Status"
        value={legality.status}
        onChange={(event) => handleOnChange({ ...legality, status: event.target.value })}
      >
        {LEGALITY_STATUSES.map((status) => (
          <option key={status} value={status}>
            {status.replace('_', ' ')}
          </option>
        ))}
      </select>
    </span>
  )
}

function ColorEditor({ value, handleOnChange }: ValueEditorProps) {
  const selected: string[] = Array.isArray(value) ? value : []
  const toggle = (color: string) =>
    handleOnChange(
      selected.includes(color) ? selected.filter((c) => c !== color) : COLORS.filter((c) => c === color || selected.includes(c)),
    )
  return (
    <span className="rule-value colors">
      {COLORS.map((color) => (
        <label key={color}>
          <input type="checkbox" checked={selected.includes(color)} onChange={() => toggle(color)} />
          {color}
        </label>
      ))}
      {selected.length === 0 && <span className="hint">colorless</span>}
    </span>
  )
}

function TagEditor({ value, handleOnChange }: ValueEditorProps) {
  const tag = value as TagValue
  const listId = useId()
  const [text, setText] = useState(tag.label)
  const suggestions = useQuery({
    queryKey: ['tags', text],
    queryFn: () => fetchTags(text),
    enabled: text.length >= 2,
    staleTime: 60_000,
  })
  const optionText = (slug: string, kind: string) => `${slug} (${kind})`

  const onText = (next: string) => {
    setText(next)
    const match = suggestions.data?.find((option) => optionText(option.slug, option.kind) === next)
    handleOnChange({ ...tag, tag_id: match?.tag_id ?? '', label: next })
  }

  return (
    <span className="rule-value">
      <input
        list={listId}
        aria-label="Tag"
        placeholder="tag slug or label"
        value={text}
        aria-invalid={text !== '' && tag.tag_id === ''}
        onChange={(event) => onText(event.target.value)}
      />
      <datalist id={listId}>
        {suggestions.data?.map((option) => (
          <option key={option.tag_id} value={optionText(option.slug, option.kind)}>
            {option.label}, {option.cards} cards
          </option>
        ))}
      </datalist>
      <label>
        <input
          type="checkbox"
          checked={tag.include_children}
          onChange={(event) => handleOnChange({ ...tag, include_children: event.target.checked })}
        />
        include child tags
      </label>
    </span>
  )
}

function KeywordEditor({ value, handleOnChange }: ValueEditorProps) {
  const listId = useId()
  const text = Array.isArray(value) ? value.join(', ') : typeof value === 'string' ? value : ''
  const last = text.split(',').at(-1)?.trim() ?? ''
  const suggestions = useQuery({
    queryKey: ['keywords', last],
    queryFn: () => fetchKeywords(last),
    enabled: last.length >= 2,
    staleTime: 60_000,
  })
  const head = text.includes(',') ? `${text.slice(0, text.lastIndexOf(',') + 1)} ` : ''
  return (
    <span className="rule-value">
      <input
        list={listId}
        aria-label="Keywords"
        placeholder="comma separated"
        value={text}
        onChange={(event) => handleOnChange(event.target.value)}
      />
      <datalist id={listId}>
        {suggestions.data?.map((keyword) => (
          <option key={keyword} value={`${head}${keyword}`} />
        ))}
      </datalist>
    </span>
  )
}
