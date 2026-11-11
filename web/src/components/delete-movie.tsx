import { useQuery } from "@connectrpc/connect-query";
import { useState } from "react";

import { ConfirmDialog } from "@/components/confirm-dialog";
import { useSensitiveMutation } from "@/components/confirm-its-you";
import { FormSelect } from "@/components/form-field";
import type { Version } from "@/gen/nama/v1/browse_pb";
import {
  type DeleteMediaResponse,
  type FileNotDeleted,
  LibraryService,
} from "@/gen/nama/v1/library_pb";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { metadataLine, sizeOf, versionLine } from "@/lib/browse";
import { formText, refusal } from "@/lib/forms";

/** The choice that deletes every Version, where any other choice is one Version's id. */
const EVERY_VERSION = "every";

/** A Movie, as deleting it needs it. */
export interface DeletableMovie {
  libraryId: bigint;
  movieId: ExternalId;
  title: string;
  versions: Version[];
}

/**
 * Deletes the Movie's every Version, or one of them, once confirmed with the files that go and the
 * space they free. A file that couldn't be deleted is listed afterwards, with why, and so is each
 * Fulfiller that couldn't be told, which may download the Movie again.
 */
export function DeleteMovie({
  onDeletedEveryVersion,
  ...movie
}: DeletableMovie & {
  /**
   * Called once every Version went, so the page can leave the Movie that's gone, with the names of
   * the Fulfillers that couldn't be told.
   */
  onDeletedEveryVersion: (fulfillersNotTold: string[]) => Promise<unknown>;
}) {
  const [result, setResult] = useState<Outcome>({ notDeleted: [], fulfillersNotTold: [] });
  const remove = useSensitiveMutation(LibraryService.method.deleteMedia);
  return (
    <>
      <ConfirmDialog
        action="Delete"
        name={`Delete ${movie.title}`}
        title={`Delete ${movie.title}?`}
        description="Its files are deleted from the Server for good. This can’t be undone."
        onConfirm={async (typed) => {
          const chosen = formText(typed, "version") || EVERY_VERSION;
          const deleted = await remove.mutateAsync({ target: targetOf(movie, chosen) });
          setResult(deleted);
          if (chosen === EVERY_VERSION && deleted.notDeleted.length === 0) {
            await onDeletedEveryVersion(deleted.fulfillersNotTold);
          }
        }}
        refusal={(error) => refusal(error, "delete it", {}).form}
      >
        <WhatGoes {...movie} />
      </ConfirmDialog>
      <NotDeleted files={result.notDeleted} />
      <MayDownloadAgain title={movie.title} fulfillers={result.fulfillersNotTold} />
    </>
  );
}

/** The choice of Version, when there are several, and the files the choice deletes. */
function WhatGoes(movie: DeletableMovie) {
  const [chosen, setChosen] = useState(EVERY_VERSION);
  return (
    <>
      {movie.versions.length > 1 && (
        <FormSelect
          label="What to delete"
          name="version"
          value={chosen}
          onChange={(event) => {
            setChosen(event.target.value);
          }}
        >
          <option value={EVERY_VERSION}>Every Version</option>
          {movie.versions.map((version) => (
            <option key={String(version.id)} value={String(version.id)}>
              {version.fileName} ({metadataLine(versionLine(version), sizeOf(version.size))})
            </option>
          ))}
        </FormSelect>
      )}
      <FilesThatGo movie={movie} chosen={chosen} />
    </>
  );
}

/** The files deleting `chosen` of `movie` takes, and the space that frees, as the Server previews it. */
function FilesThatGo({ movie, chosen }: { movie: DeletableMovie; chosen: string }) {
  const { data, error } = useQuery(LibraryService.method.previewDeleteMedia, {
    target: targetOf(movie, chosen),
  });
  if (error) {
    return <p className="text-sm text-destructive">{refusal(error, "find its files", {}).form}</p>;
  }
  if (!data) return <p className="text-sm text-muted-foreground">Finding its files…</p>;
  return (
    <div className="text-sm">
      <p>
        {data.files.length === 1 ? "This file goes" : `These ${data.files.length} files go`},
        freeing {sizeOf(data.bytesFreed)}:
      </p>
      <ul className="mt-2 flex max-h-48 flex-col gap-1 overflow-y-auto text-muted-foreground">
        {/* Versions in different folders can have files named alike, so a name isn't a key. */}
        {data.files.map((file, index) => (
          <li key={`${String(index)} ${file.fileName}`} className="break-all">
            {file.fileName}
          </li>
        ))}
      </ul>
    </div>
  );
}

/** The files the last delete couldn't delete, each with why. */
function NotDeleted({ files }: { files: FileNotDeleted[] }) {
  return (
    <div role="alert" className="basis-full text-sm empty:hidden">
      {files.length > 0 && (
        <>
          <p className="text-destructive">
            {files.length === 1 ? "A file" : `${files.length} files`} couldn’t be deleted. The rest
            went.
          </p>
          <ul className="mt-1 flex flex-col gap-1 text-muted-foreground">
            {files.map((file, index) => (
              <li key={`${String(index)} ${file.fileName}`} className="break-all">
                {file.fileName}: {file.reason}
              </li>
            ))}
          </ul>
        </>
      )}
    </div>
  );
}

/** Says which Fulfillers couldn't be told a delete of `title` happened, so they may download it again. */
export function MayDownloadAgain({ title, fulfillers }: { title: string; fulfillers: string[] }) {
  return (
    <output className="block basis-full text-sm text-pretty empty:hidden">
      {fulfillers.length > 0 &&
        `${names.format(fulfillers)} couldn’t be told about the delete, so ${fulfillers.length === 1 ? "it" : "they"} may download ${title} again.`}
    </output>
  );
}

// "en-GB" for "Radarr and Sonarr", without the serial comma, as the rest of the app writes lists.
const names = new Intl.ListFormat("en-GB", { type: "conjunction" });

/** What the last delete reports: the files it couldn't delete and the Fulfillers it couldn't tell. */
type Outcome = Pick<DeleteMediaResponse, "notDeleted" | "fulfillersNotTold">;

/** What deleting `chosen` of `movie` names: every Version, or the one with that id. */
function targetOf({ libraryId, movieId }: DeletableMovie, chosen: string) {
  return chosen === EVERY_VERSION
    ? { target: { case: "movie" as const, value: { libraryId, movieId } } }
    : { target: { case: "versionId" as const, value: BigInt(chosen) } };
}
