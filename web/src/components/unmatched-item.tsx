import { Code } from "@connectrpc/connect";
import { useMutation, useQuery } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";
import { useId, useState } from "react";

import { FormField } from "@/components/form-field";
import { Button } from "@/components/ui/button";
import {
  MatchService,
  type TitleCandidate,
  TitleKind,
  type UnmatchedItem,
} from "@/gen/nama/v1/match_pb";
import { unmatchedKey } from "@/lib/events";
import { formText, onSubmitted, refusal } from "@/lib/forms";

/**
 * An Unmatched Item: what its file's name was parsed as, and the file. Opening it searches the
 * metadata source for its title, to Match it by hand.
 */
export function UnmatchedRow({
  item,
  libraryId,
  open,
  onToggle,
}: {
  item: UnmatchedItem;
  libraryId: bigint;
  open: boolean;
  onToggle: () => void;
}) {
  const panel = useId();
  const parsed = parsedAs(item);
  return (
    <li className="border-b px-3 py-3 last:border-b-0">
      <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
        <div className="flex min-w-0 flex-1 flex-col">
          <span className="font-medium">
            {parsed}
            <span className="ms-2 text-sm font-normal text-muted-foreground">
              {item.kind === TitleKind.SHOW ? "Episode" : "Movie"}
            </span>
          </span>
          <span className="text-sm wrap-anywhere text-muted-foreground">{item.path}</span>
        </div>
        <Button
          variant="ghost"
          aria-label={`Find a Match for ${parsed}`}
          aria-expanded={open}
          aria-controls={panel}
          onClick={onToggle}
        >
          {open ? "Close" : "Find a Match"}
        </Button>
      </div>
      {open && (
        <div id={panel} className="pt-4 pb-3">
          <FindAMatch item={item} libraryId={libraryId} />
        </div>
      )}
    </li>
  );
}

/** What an Item's file name was parsed as, such as "Alien (1979)" or "Severance · Season 1, Episode 2". */
function parsedAs(item: UnmatchedItem) {
  const title = withYear(item.title, item.year);
  if (item.season === undefined || item.episode === undefined) return title;
  return `${title} · Season ${item.season}, Episode ${item.episode}`;
}

function withYear(title: string, year: number | undefined) {
  return year === undefined ? title : `${title} (${year})`;
}

function FindAMatch({ item, libraryId }: { item: UnmatchedItem; libraryId: bigint }) {
  const [search, setSearch] = useState({ kind: item.kind, title: item.title, year: item.year });
  return (
    <div className="flex max-w-xl flex-col gap-4">
      <form
        className="flex flex-wrap items-end gap-3"
        onSubmit={onSubmitted((typed) => {
          setSearch(searchTyped(typed, item.kind));
        })}
      >
        <div className="min-w-48 flex-1">
          <FormField label="Title" name="title" autoComplete="off" defaultValue={item.title} />
        </div>
        <div className="w-24">
          <FormField
            label="Year"
            name="year"
            type="number"
            autoComplete="off"
            required={false}
            defaultValue={item.year}
          />
        </div>
        <Button type="submit">Search</Button>
      </form>
      <SearchResults search={search} item={item} libraryId={libraryId} />
    </div>
  );
}

function searchTyped(typed: FormData, kind: TitleKind) {
  const year = formText(typed, "year");
  return { kind, title: formText(typed, "title"), year: year === "" ? undefined : Number(year) };
}

/** The titles the metadata source found for `search`, to Match `item` to. */
function SearchResults({
  search,
  item,
  libraryId,
}: {
  search: ReturnType<typeof searchTyped>;
  item: UnmatchedItem;
  libraryId: bigint;
}) {
  const found = useQuery(MatchService.method.searchMetadata, search);
  if (found.isError) {
    return (
      <p role="alert" className="text-sm text-destructive">
        {searchRefusal(found.error)}
      </p>
    );
  }
  if (found.data === undefined) {
    return <p className="text-sm text-muted-foreground">Searching…</p>;
  }
  const { candidates } = found.data;
  if (candidates.length === 0) {
    return (
      <p className="text-sm text-muted-foreground">Nothing found. Try another title or year.</p>
    );
  }
  return <Candidates candidates={candidates} item={item} libraryId={libraryId} />;
}

function Candidates({
  candidates,
  item,
  libraryId,
}: {
  candidates: TitleCandidate[];
  item: UnmatchedItem;
  libraryId: bigint;
}) {
  const queryClient = useQueryClient();
  // Matching an Episode's Show may Unmatch its other Episodes, which no Event announces.
  const setMatch = useMutation(MatchService.method.setMatch, {
    onSuccess: () => queryClient.invalidateQueries({ queryKey: unmatchedKey(libraryId) }),
  });
  return (
    <>
      <ul aria-label="Titles found" className="-mx-3 flex flex-col">
        {candidates.map((candidate) => (
          <CandidateRow
            key={`${candidate.id?.id.case}:${candidate.id?.id.value}`}
            candidate={candidate}
            pending={setMatch.isPending}
            onMatch={() => {
              setMatch.mutate({ itemId: item.itemId, titleId: candidate.id });
            }}
          />
        ))}
      </ul>
      <p role="alert" className="text-sm text-destructive empty:hidden">
        {setMatch.error && setMatchRefusal(setMatch.error)}
      </p>
    </>
  );
}

function CandidateRow({
  candidate,
  pending,
  onMatch,
}: {
  candidate: TitleCandidate;
  pending: boolean;
  onMatch: () => void;
}) {
  const name = withYear(candidate.title, candidate.year);
  const { originalTitle } = candidate;
  return (
    <li className="flex items-center gap-3 rounded-lg px-3 py-1 hover:bg-muted">
      <span className="min-w-0 flex-1">
        {name}
        {originalTitle !== undefined && originalTitle !== candidate.title && (
          <span className="ms-2 text-sm text-muted-foreground">{originalTitle}</span>
        )}
      </span>
      <Button variant="ghost" aria-label={`Match to ${name}`} disabled={pending} onClick={onMatch}>
        Match
      </Button>
    </li>
  );
}

const noAnswer = { form: "The metadata source didn’t answer. Try again in a moment." };

function searchRefusal(error: unknown) {
  return refusal(error, "search", { [Code.Unavailable]: noAnswer }).form;
}

function setMatchRefusal(error: unknown) {
  return refusal(error, "set the Match", {
    [Code.Unavailable]: noAnswer,
    [Code.NotFound]: { form: "This file is gone from the Library." },
  }).form;
}
