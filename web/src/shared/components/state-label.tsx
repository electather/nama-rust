import type { LucideIcon } from "lucide-react";

/** A state as a labelled pill shows it. */
export interface Shown {
  label: string;
  Icon: LucideIcon;
}

const TONES = {
  quiet: "bg-secondary text-foreground",
  // It's as it should be, such as Connected or Saved: lit, as a chosen thing is.
  positive: "bg-accent text-primary",
  // Something gave way, such as a Playback Limit the player couldn't keep: it stands out of the
  // picture without the alarm of Destructive.
  warning: "bg-secondary text-foreground",
  // Declined or Failed: DESIGN.md keeps Destructive the only status colour, so no other state uses it.
  destructive: "bg-destructive/15 text-destructive",
};

const SIZES = {
  default: { pill: "gap-1.5 px-2.5 py-0.5 text-sm", icon: "size-3.5" },
  // Beside a title's Play, as tall as it.
  large: { pill: "h-11 gap-2 ps-4.5 pe-5.5 text-base font-medium", icon: "size-4.5" },
};

/** A state as a labelled pill, with an icon so it never rests on colour alone. */
export function StateLabel({
  label,
  Icon,
  tone = "quiet",
  large = false,
}: Shown & { tone?: keyof typeof TONES; large?: boolean }) {
  const size = large ? "large" : "default";
  return (
    <span
      className={`inline-flex shrink-0 items-center rounded-full ${SIZES[size].pill} ${TONES[tone]}`}
    >
      <Icon aria-hidden className={`shrink-0 ${SIZES[size].icon}`} />
      {label}
    </span>
  );
}
