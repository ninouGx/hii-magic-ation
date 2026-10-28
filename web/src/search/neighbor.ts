/** The oracle_id `step` places away in the loaded result, or null past either end. */
export function neighbor(cards: { oracle_id: string }[], current: string, step: -1 | 1): string | null {
  const index = cards.findIndex((card) => card.oracle_id === current)
  if (index === -1) return null
  return cards[index + step]?.oracle_id ?? null
}
