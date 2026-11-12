import type { MessageInitShape } from "@bufbuild/protobuf";
import { useQuery } from "@connectrpc/connect-query";
import { useState } from "react";

import { ConfirmDialog } from "@/components/confirm-dialog";
import { useSensitiveMutation } from "@/components/confirm-its-you";
import { FormSelect } from "@/components/form-field";
import type { Version } from "@/gen/nama/v1/browse_pb";
import {
  type DeleteMediaResponse,
  type EpisodeTakenAlong,
  type FileNotDeleted,
  LibraryService,
  type MediaToDeleteSchema,
} from "@/gen/nama/v1/library_pb";
import { metadataLine, seasonName, sizeOf, versionLine } from "@/lib/browse";
import { formText, refusal } from "@/lib/forms";

/** The choice that deletes every Version, where any other choice is one Version's id. */
const EVERY_VERSION = "every";

/** A Movie, Show, Season or Episode, as deleting it needs it. */
export interface Deletable {
  /** What the dialog asks about deleting, such as the Movie's title. */
  title: string;
  /** Its Versions, to choose one of when there are several. A Show or Season offers none. */
  versions: Version[];
  /** What deleting every Version names, or with an id, only that Version. */
  target: (versionId?: bigint) => MessageInitShape<typeof MediaToDeleteSchema>;
}

/**
 * Deletes a title's every Version, or one of them, once confirmed with the files that go, the
 * space they free and the other Episodes those files hold. A file that couldn't be deleted is
 * listed afterwards, with why.
 */
export function DeleteMedia({
  action = "Delete",
  name = action,
  onDeletedEveryVersion,
  ...deletable
}: Deletable & {
  /** What the button says, such as "Delete Show" where "Delete" alone wouldn't say what goes. */
  action?: string;
  /** The button's accessible name, where `action` alone doesn't say what goes, such as "Delete Alien". */
  name?: string;
  /**
   * Called once every Version went, so the page can leave what's gone, with the names of the
   * Fulfillers that couldn't be told, which the page then reports.
   */
  onDeletedEveryVersion: (fulfillersNotTold: string[]) => Promise<unknown>;
}) {
  const [result, setResult] = useState<Outcome>({ notDeleted: [], fulfillersNotTold: [] });
  const remove = useSensitiveMutation(LibraryService.method.deleteMedia);
  return (
    <>
      <ConfirmDialog
        action={action}
        name={name}
        title={`Delete ${deletable.title}?`}
        description="Its files are deleted from the Server for good. This can’t be undone."
        onConfirm={async (typed) => {
          const chosen = formText(typed, "version") || EVERY_VERSION;
          const deleted = await remove.mutateAsync({ target: targetOf(deletable, chosen) });
          if (chosen === EVERY_VERSION && deleted.notDeleted.length === 0) {
            // This may stay mounted for another title, such as the next Season, so the page reports.
            setResult({ notDeleted: [], fulfillersNotTold: [] });
            await onDeletedEveryVersion(deleted.fulfillersNotTold);
            return;
          }
          setResult(deleted);
        }}
        refusal={(error) => refusal(error, "delete it", {}).form}
      >
        <WhatGoes {...deletable} />
      </ConfirmDialog>
      <NotDeleted files={result.notDeleted} />
      <MayDownloadAgain title={deletable.title} fulfillers={result.fulfillersNotTold} />
    </>
  );
}

/** The choice of Version, when there are several, and the files the choice deletes. */
function WhatGoes(deletable: Deletable) {
  const [chosen, setChosen] = useState(EVERY_VERSION);
  return (
    <>
      {deletable.versions.length > 1 && (
        <FormSelect
          label="What to delete"
          name="version"
          value={chosen}
          onChange={(event) => {
            setChosen(event.target.value);
          }}
        >
          <option value={EVERY_VERSION}>Every Version</option>
          {deletable.versions.map((version) => (
            <option key={String(version.id)} value={String(version.id)}>
              {version.fileName} ({metadataLine(versionLine(version), sizeOf(version.size))})
            </option>
          ))}
        </FormSelect>
      )}
      <FilesThatGo deletable={deletable} chosen={chosen} />
    </>
  );
}

/**
 * The files deleting `chosen` of `deletable` takes, the space that frees, the other Episodes
 * those files hold and the read-only folder that refuses it if a scan found one since the page
 * loaded, as the Server previews it.
 */
function FilesThatGo({ deletable, chosen }: { deletable: Deletable; chosen: string }) {
  const { data, error } = useQuery(LibraryService.method.previewDeleteMedia, {
    target: targetOf(deletable, chosen),
  });
  if (error) {
    return <p className="text-sm text-destructive">{refusal(error, "find its files", {}).form}</p>;
  }
  if (!data) return <p className="text-sm text-muted-foreground">Finding its files…</p>;
  return (
    <div className="flex flex-col gap-3 text-sm">
      <div>
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
      <TakenAlong episodes={data.episodesTakenAlong} />
      <Watching playing={data.playing} />
      <RefusedFor folder={data.readOnlyFolder} />
    </div>
  );
}

/** Says the Server will refuse the delete because it can't write to `folder`, if there is one. */
function RefusedFor({ folder }: { folder: string | undefined }) {
  if (folder === undefined) return null;
  return (
    <p className="text-destructive">
      The Server will refuse to delete them: it can’t write to {folder}.
    </p>
  );
}

/** How many people deleting it stops watching, if anyone. */
function Watching({ playing }: { playing: number }) {
  if (playing === 0) return null;
  return (
    <p className="font-medium">
      {playing === 1 ? "1 person is" : `${playing} people are`} watching it now. Deleting it stops
      their playback.
    </p>
  );
}

/** The other Episodes the files that go hold, which lose them too. */
function TakenAlong({ episodes }: { episodes: EpisodeTakenAlong[] }) {
  if (episodes.length === 0) return null;
  return (
    <div>
      <p>A file that goes also holds:</p>
      <ul className="mt-2 flex flex-col gap-1 text-muted-foreground">
        {episodes.map((episode) => (
          <li key={`${String(episode.seasonNumber)} ${String(episode.episodeNumber)}`}>
            {seasonName(episode.seasonNumber)}, Episode {episode.episodeNumber}: {episode.title}
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

/** What deleting `chosen` of `deletable` names: every Version, or the one with that id. */
function targetOf({ target }: Deletable, chosen: string) {
  return chosen === EVERY_VERSION ? target() : target(BigInt(chosen));
}
