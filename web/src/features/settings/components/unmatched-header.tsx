import { useEffect, useRef } from "react";

import { reasonSentence } from "@/features/admin-needs-a-match/lib/unmatched-reason";
import { FileDetails } from "@/features/settings/components/file-details";
import { FilePoster } from "@/features/settings/components/file-poster";
import type { UnmatchedItem } from "@/gen/nama/v1/match_pb";
import { PlayUnmatched } from "@/shared/components/play-unmatched";
import { parsedAs } from "@/shared/lib/unmatched-names";

/**
 * The top of a file's own screen: its stand-in poster, what its name was parsed as in a large
 * headline (`titleId`), why it needs a Match as a sentence, and Play to check it is the file it
 * seems, with its details, the Library `libraryId`'s.
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
    <header className="flex flex-wrap items-start gap-x-3.5 gap-y-4 md:gap-x-5 md:gap-y-2">
      <FilePoster kind={item.kind} size="header" />
      <div className="flex min-w-0 flex-1 basis-0 flex-col gap-1.5 md:gap-2">
        <h3
          ref={title}
          id={titleId}
          tabIndex={-1}
          className="text-2xl font-medium tracking-tight text-balance outline-none md:text-3xl"
        >
          {parsed}
        </h3>
        <p className="max-w-prose text-sm text-pretty text-muted-foreground md:text-base">
          {reasonSentence(item)}
        </p>
      </div>
      {/* Under the name from `md`, past the poster; on a phone, under both. */}
      <div className="flex basis-full flex-wrap items-center gap-2 md:ms-25">
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
