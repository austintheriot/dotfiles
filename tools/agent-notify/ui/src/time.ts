const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

export function relativeTime(createdMs: number, nowMs: number): string {
  const elapsed = Math.max(0, nowMs - createdMs);
  if (elapsed < 45_000) return "now";
  if (elapsed < HOUR) return `${Math.max(1, Math.round(elapsed / MINUTE))}m`;
  if (elapsed < DAY) return `${Math.floor(elapsed / HOUR)}h`;
  return `${Math.floor(elapsed / DAY)}d`;
}
