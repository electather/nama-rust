import type { LucideIcon } from "lucide-react";

/** A state as a labelled pill shows it. */
export interface Shown {
  label: string;
  Icon: LucideIcon;
}

/** A state as a labelled pill, with an icon so it never rests on colour alone. */
export function StateLabel({ label, Icon }: Shown) {
  return (
    <span className="inline-flex shrink-0 items-center gap-1.5 rounded-full border px-2.5 py-0.5 text-sm text-muted-foreground">
      <Icon aria-hidden className="size-3.5" />
      {label}
    </span>
  );
}
