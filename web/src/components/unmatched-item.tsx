import { Code } from "@connectrpc/connect";
import { useMutation, useQuery } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { Play } from "lucide-react";
import { useState } from "react";

import {
  MatchService,
  type SetMatchResponse,
  type TitleCandidate,
  type TitleKind,
  type UnmatchedItem,
} from "@/gen/nama/v1/match_pb";
import { browseKey } from "@/lib/browse";
// oxlint-disable-next-line import/max-dependencies -- an Unmatched Item is played, searched for and Matched from here
import { unmatchedKey } from "@/lib/events";
import { FormField } from "@/shared/components/form-field/form-field";
import { formText, onSubmitted, refusal } from "@/shared/lib/forms";
import { episodeName } from "@/shared/lib/title-facts";
import { Button, buttonVariants } from "@/shared/ui/button";

/** Plays the Version `versionId` of the Library `libraryId`, which holds the Unmatched Item `name`. */
export function PlayUnmatched({
  libraryId,
  versionId,
  name,
}: {
  libraryId: bigint;
  versionId: bigint;
  name: string;
}) {
  return (
    <Link
      to="/libraries/$libraryId/unmatched/$versionId/play"
      params={{ libraryId: String(libraryId), versionId: String(versionId) }}
      aria-label={`Play ${name}`}
      className={buttonVariants({ variant: "ghost" })}
    >
      <Play aria-hidden />
      Play
    </Link>
  );
}

/**
 * What an Item's file name was parsed as, such as "Alien (1979)", "Severance · Season 1, Episode 2"
 * or "Severance · Specials, Episode 2".
 */
export function parsedAs(item: Pick<UnmatchedItem, "title" | "year" | "season" | "episode">) {
  const title = withYear(item.title, item.year);
  if (item.season === undefined || item.episode === undefined) {
    return title;
  }
  return `${title} · ${episodeName(item.season, item.episode, "full")}`;
}

/** `title`, followed by `year` in brackets if it's known, such as "Alien (1979)". */
export function withYear(title: string, year: number | undefined) {
  return year === undefined ? title : `${title} (${year})`;
}

/** What Matching an Item by hand needs of it: which Item, and what its file's name was parsed as. */
export type ItemToMatch = Pick<
  UnmatchedItem,
  "itemId" | "kind" | "title" | "year" | "season" | "episode"
>;

/** Searches the title source for `item`'s title, and Matches it to the title picked. */
export function FindAMatch({ item, libraryId }: { item: ItemToMatch; libraryId: bigint }) {
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

/** The titles the title source found for `search`, to Match `item` to. */
function SearchResults({
  search,
  item,
  libraryId,
}: {
  search: ReturnType<typeof searchTyped>;
  item: ItemToMatch;
  libraryId: bigint;
}) {
  const found = useQuery(MatchService.method.searchTitles, search);
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
  item: ItemToMatch;
  libraryId: bigint;
}) {
  const queryClient = useQueryClient();
  // Matching an Episode's Show may Unmatch its other Episodes, which no Event announces. Browsing
  // refetches too, so a player of the Item sees it Matched without waiting for the Event.
  const setMatch = useMutation(MatchService.method.setMatch, {
    onSuccess: () =>
      Promise.all([
        queryClient.invalidateQueries({ queryKey: unmatchedKey(libraryId) }),
        queryClient.invalidateQueries({ queryKey: browseKey }),
      ]),
  });
  return (
    <>
      <ul aria-label="Titles found" className="-mx-3 flex flex-col">
        {candidates.map((candidate) => (
          <CandidateRow
            key={`${candidate.id?.namespace}:${candidate.id?.value}`}
            candidate={candidate}
            pending={setMatch.isPending}
            onMatch={() => {
              setMatch.mutate({ itemId: item.itemId, titleId: candidate.id });
            }}
          />
        ))}
      </ul>
      <output className="block text-sm text-pretty empty:hidden">
        {setMatch.data && notInShow(item, setMatch.data)}
      </output>
      <p role="alert" className="text-sm text-destructive empty:hidden">
        {setMatch.error && setMatchRefusal(setMatch.error)}
      </p>
    </>
  );
}

/**
 * What Matching `item`, an Episode, to a Show without it did, or "" if the Show has it. The row
 * closes once its Item is Matched, so it shows this only while its own Episode is still Unmatched.
 */
function notInShow(item: ItemToMatch, { matched, unmatched }: SetMatchResponse) {
  if (unmatched === 0n || item.season === undefined || item.episode === undefined) {
    return "";
  }
  return `That Show has no ${episodeName(item.season, item.episode, "full")}.${alsoMatched(matched)}`;
}

function alsoMatched(othersMatched: bigint) {
  if (othersMatched === 0n) {
    return "";
  }
  const others = othersMatched === 1n ? "1 other Episode" : `${othersMatched} other Episodes`;
  return ` ${others} Matched to it.`;
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

const noAnswer = { form: "The title source didn’t answer. Try again in a moment." };

function searchRefusal(error: unknown) {
  return refusal(error, "search", { [Code.Unavailable]: noAnswer }).form;
}

function setMatchRefusal(error: unknown) {
  return refusal(error, "set the Match", {
    [Code.Unavailable]: noAnswer,
    [Code.NotFound]: { form: "This file is gone from the Library." },
  }).form;
}
