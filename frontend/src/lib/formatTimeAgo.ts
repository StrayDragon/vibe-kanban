export function formatTimeAgo(value: string | Date): string {
  const date = value instanceof Date ? value : new Date(value);
  if (Number.isNaN(date.getTime())) return '';

  const diffMs = Date.now() - date.getTime();
  const absSec = Math.round(Math.abs(diffMs) / 1000);

  const rtf =
    typeof Intl !== 'undefined' && typeof Intl.RelativeTimeFormat === 'function'
      ? new Intl.RelativeTimeFormat(undefined, { numeric: 'auto' })
      : null;

  const to = (value: number, unit: Intl.RelativeTimeFormatUnit) =>
    rtf
      ? rtf.format(-value, unit)
      : `${value} ${unit}${value !== 1 ? 's' : ''} ago`;

  if (absSec < 60) return to(Math.round(absSec), 'second');
  const mins = Math.round(absSec / 60);
  if (mins < 60) return to(mins, 'minute');
  const hours = Math.round(mins / 60);
  if (hours < 24) return to(hours, 'hour');
  const days = Math.round(hours / 24);
  if (days < 30) return to(days, 'day');
  const months = Math.round(days / 30);
  if (months < 12) return to(months, 'month');
  const years = Math.round(months / 12);
  return to(years, 'year');
}
