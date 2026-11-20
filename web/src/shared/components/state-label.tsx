import type { LucideIcon } from "lucide-react";

/** A state as a labelled pill shows it. */
export interface Shown {
  label: string;
  Icon: LucideIcon;
}

const TONES = {
  quiet: "border text-muted-foreground",
  // Something gave way, such as a Playback Limit the player couldn't keep: it stands out of the
  // picture without the alarm of Destructive.
  warning: "bg-secondary text-foreground",
  // A Request Declined or Failed: the one state red, as nothing else is.
  destructive: "bg-destructive/15 text-destructive",
};

/** A state as a labelled pill, with an icon so it never rests on colour alone. */
export function StateLabel({ label, Icon, tone = "quiet" }: Shown & { tone?: keyof typeof TONES }) {
  return (
    <span
      className={`inline-flex shrink-0 items-center gap-1.5 rounded-full px-2.5 py-0.5 text-sm ${TONES[tone]}`}
    >
      <Icon aria-hidden className="size-3.5 shrink-0" />
      {label}
    </span>
  );
}
