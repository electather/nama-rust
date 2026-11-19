import { ChevronLeft } from "lucide-react";

/** The first row of a list Playback settings opens: its `title`, which goes back to settings. */
export function BackRow({ title, onBack }: { title: string; onBack: () => void }) {
  return (
    <button
      type="button"
      aria-label={`${title}, back to settings`}
      onClick={onBack}
      className="flex h-10 w-full shrink-0 cursor-pointer items-center gap-2.5 rounded-md px-2.5 text-start font-semibold transition-colors outline-none select-none hover:bg-muted focus-visible:bg-muted focus-visible:ring-3 focus-visible:ring-ring/50"
    >
      <ChevronLeft aria-hidden className="size-4" />
      {title}
    </button>
  );
}
