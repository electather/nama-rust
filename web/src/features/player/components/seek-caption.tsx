import { clock } from "@/shared/lib/clock";

/** The hover card's caption: the section's `name` and the time, or with `timeOnly`, the time alone. */
export function SeekCaption({
  name,
  seconds,
  timeOnly,
}: {
  name: string | undefined;
  seconds: number;
  timeOnly: boolean;
}) {
  const time = <span className="shrink-0 text-xs tabular-nums">{clock(seconds)}</span>;
  if (timeOnly) {
    return time;
  }
  return (
    <span className="flex items-baseline justify-between gap-2 px-0.5">
      <span className="min-w-0 truncate text-sm font-semibold">{name}</span>
      {time}
    </span>
  );
}
