import type { WatchState } from "@/gen/nama/v1/watch_state_pb";
import { ResumeBar } from "@/shared/components/watch-state/resume-bar";
import { useSrcSet } from "@/shared/hooks/use-src-set";

/** The `sizes` of a Title Logo on a `BackdropCard`: up to 60% of its width. */
const cardLogoSizes = "(min-width: 1024px) 189px, (min-width: 640px) 30vw, 60vw";

/**
 * A card's title, as its Title Logo named by the title or as text without one, and its Watch State,
 * over the foot of its art, on the scrim, which fades in over art.
 */
export function CardCaption({
  art,
  title,
  logoUrl,
  watchState,
}: {
  art: boolean;
  title: string;
  logoUrl: string | undefined;
  watchState: WatchState | undefined;
}) {
  const srcSet = useSrcSet(logoUrl);
  return (
    <div className="relative w-full">
      {art && <div className="h-8 bg-linear-to-t from-scrim" />}
      {/* On the scrim's full strength however many lines it takes, under a fade into the art. */}
      <div className="px-4 pb-3 group-data-art:bg-scrim">
        {logoUrl === undefined ? (
          <p className="text-2xl font-semibold tracking-tight text-balance text-foreground">
            {title}
          </p>
        ) : (
          <img
            src={logoUrl}
            srcSet={srcSet}
            sizes={cardLogoSizes}
            alt={title}
            className="max-h-12 w-auto max-w-3/5 object-contain"
          />
        )}
        <ResumeBar state={watchState} />
      </div>
    </div>
  );
}
