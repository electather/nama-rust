import { ChevronRight, type LucideIcon } from "lucide-react";

/** A row of the Playback settings panel, saying what it's set to when it opens a list. */
export function SettingsRow({
  Icon,
  label,
  value,
  onClick,
}: {
  Icon: LucideIcon;
  label: string;
  value?: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="group/row flex h-10 w-full shrink-0 cursor-pointer items-center gap-2.5 rounded-md px-2.5 text-start transition-colors outline-none select-none hover:bg-muted focus-visible:bg-muted focus-visible:ring-3 focus-visible:ring-ring/50"
    >
      <Icon aria-hidden className="size-4 text-muted-foreground group-hover/row:text-foreground" />
      <span className="flex-1">{label}</span>
      {value !== undefined && (
        <span className="text-xs text-muted-foreground group-hover/row:text-foreground">
          {value}
        </span>
      )}
      {value !== undefined && (
        <ChevronRight
          aria-hidden
          className="size-4 text-muted-foreground group-hover/row:text-foreground"
        />
      )}
    </button>
  );
}
