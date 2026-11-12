import { useSuspenseQuery } from "@connectrpc/connect-query";
import { createFileRoute } from "@tanstack/react-router";
import { type ReactNode, useState } from "react";

import { MovieActions } from "@/components/movie-actions";
import { NotFound } from "@/components/route-error";
import { Overview, TitleFacts, TitlePage } from "@/components/title-page";
import { PlayOrResume } from "@/components/watch-state";
import { BrowseService, HdrFormat, type Version } from "@/gen/nama/v1/browse_pb";
import type { WatchState } from "@/gen/nama/v1/watch_state_pb";
import {
  loadMovie,
  metadataLine,
  runtimeOf,
  sizeOf,
  timeLeft,
  versionLine,
  versionToPlay,
  yearOf,
} from "@/lib/browse";

export const Route = createFileRoute("/_member/libraries/$libraryId/movies/$movieId/")({
  loader: ({ context, params }) => loadMovie(context, params),
  notFoundComponent: NotFound,
  component: Movie,
});

function Movie() {
  const input = Route.useLoaderData();
  const { data } = useSuspenseQuery(BrowseService.method.getMovie, input);
  const [chosen, setChosen] = useState<number>();
  const { movie } = data;
  if (!movie) return <NotFound />;
  // The largest until one is chosen, and again if an Event removed the chosen one.
  const version = versionToPlay(movie.versions, chosen);
  return (
    <TitlePage
      title={movie.title}
      logoUrl={movie.titleLogoUrl}
      backdropUrl={movie.backdropUrl}
      backdropThumbhash={movie.backdropThumbhash}
    >
      <TitleFacts
        facts={[
          "Movie",
          ...movie.genres,
          yearOf(movie.releaseDate),
          runtimeOf(movie.runtimeMinutes, "short"),
        ]}
        boxed={[movie.contentRating, ...movieBadges(movie.versions)]}
      />
      <Play
        versions={movie.versions}
        version={version}
        watchState={movie.watchState}
        onChoose={setChosen}
      >
        <MovieActions
          libraryId={input.libraryId}
          movieId={input.movieId}
          movie={movie}
          readOnlyFolder={data.readOnlyFolder}
        />
      </Play>
      {movie.overview !== undefined && <Overview text={movie.overview} />}
    </TitlePage>
  );
}

/**
 * Play `version`, or Resume it with how far in you are once started, and a choice of Version, each
 * with its picture and sound, when there's more than one, then `children`.
 */
function Play({
  versions,
  version,
  watchState,
  onChoose,
  children,
}: {
  versions: Version[];
  version: Version;
  watchState: WatchState | undefined;
  onChoose: (versionId: number) => void;
  children: ReactNode;
}) {
  const versionId = String(version.id);
  const params = Route.useParams();
  return (
    <div className="mt-6 flex flex-wrap items-center gap-x-4 gap-y-3">
      <PlayOrResume
        link={{
          to: "/libraries/$libraryId/movies/$movieId/play",
          params,
          search: { version: Number(versionId) },
        }}
        state={watchState}
        detail={timeLeft(watchState)}
      />
      {versions.length > 1 && (
        <label className="flex items-center gap-2 text-sm text-muted-foreground">
          Version
          <select
            value={versionId}
            onChange={(event) => {
              onChoose(Number(event.target.value));
            }}
            className="h-9 max-w-64 truncate rounded-lg border border-input bg-transparent px-3 text-foreground outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/25"
          >
            {versions.map((option) => (
              <option key={String(option.id)} value={String(option.id)}>
                {option.fileName} ({metadataLine(versionLine(option), sizeOf(option.size))})
              </option>
            ))}
          </select>
        </label>
      )}
      {children}
    </div>
  );
}

/**
 * What any of a Movie's `versions` offers, by the names on its page: "4K" for video 3840 wide or
 * 2160 tall, "Dolby Vision" or else "HDR" from the video's `hdr`, and "SDH" for SDH subtitles.
 */
function movieBadges(versions: Version[]) {
  const tracks = versions.flatMap((version) => version.tracks);
  const videos = tracks.flatMap(({ kind }) => (kind.case === "video" ? [kind.value] : []));
  return [
    videos.some(({ width, height }) => width >= 3840 || height >= 2160) ? "4K" : undefined,
    hdrBadge(new Set(videos.map(({ hdr }) => hdr))),
    tracks.some(({ kind, sdh }) => kind.case === "subtitle" && sdh) ? "SDH" : undefined,
  ];
}

function hdrBadge(formats: Set<HdrFormat>) {
  if (formats.has(HdrFormat.DOLBY_VISION)) return "Dolby Vision";
  return formats.has(HdrFormat.HDR10) || formats.has(HdrFormat.HLG) ? "HDR" : undefined;
}
