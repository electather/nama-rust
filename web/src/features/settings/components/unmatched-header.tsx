import { useEffect, useRef } from "react";

import { reasonSentence } from "@/features/admin-needs-a-match/lib/unmatched-reason";
import { FileDetails } from "@/features/settings/components/file-details";
import type { UnmatchedItem } from "@/gen/nama/v1/match_pb";
import { PlayUnmatched } from "@/shared/components/play-unmatched";
import { parsedAs } from "@/shared/lib/unmatched-names";

/**
 * The top of a file's own screen: why it needs a Match as a sentence, and Play to check it is the
 * file it seems, with its details, the Library `libraryId`'s. The crumbs over the pane name it, so
 * here its title (`titleId`) is only said.
 */
export function UnmatchedHeader({
  titleId,
  item,
  libraryId,
}: {
  titleId: string;
  item: UnmatchedItem;
  libraryId: bigint;
}) {
  const title = useRef<HTMLHeadingElement>(null);
  useEffect(() => {
    // The list this replaced held focus, so it moves to what took its place.
    title.current?.focus();
  }, []);
  const parsed = parsedAs(item);
  return (
    <header className="flex flex-col gap-2">
      <h3 ref={title} id={titleId} tabIndex={-1} className="sr-only">
        {parsed}
      </h3>
      <p className="max-w-prose text-sm text-pretty text-muted-foreground">
        {reasonSentence(item)}
      </p>
      <div className="flex flex-wrap items-center gap-2">
        <PlayUnmatched
          libraryId={libraryId}
          versionId={item.versionId}
          name={parsed}
          variant="secondary"
          label="Play to check"
        />
        <FileDetails path={item.path} />
      </div>
    </header>
  );
}
