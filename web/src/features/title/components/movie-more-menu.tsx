import { type ComponentProps, useRef } from "react";

import { DeleteMovie } from "@/features/title/components/delete-movie";
import { EditImages } from "@/features/title/components/edit-images";
import { FixMatch } from "@/features/title/components/fix-match";
import { MoreActions } from "@/features/title/components/more-actions";
import { MovieMenuItems } from "@/features/title/components/movie-menu-items";
import { movieImages } from "@/features/title/lib/title-images";
import { titleToMatch } from "@/features/title/lib/title-to-match";
import { TitleKind } from "@/gen/nama/v1/match_pb";
import type { Opener } from "@/shared/lib/opener";

/**
 * A Movie's "…" menu, with the dialogs its items open beside it, so they stay once the menu
 * closes.
 */
export function MovieMoreMenu(
  props: Omit<ComponentProps<typeof MovieMenuItems>, "images" | "fix" | "unmatch" | "remove">,
) {
  const images = useRef<Opener>(null);
  const fix = useRef<Opener>(null);
  const unmatch = useRef<Opener>(null);
  const remove = useRef<Opener>(null);
  const { movie, movieId, libraryId } = props;
  const toMatch = titleToMatch({ libraryId, id: movieId }, TitleKind.MOVIE, {
    title: movie.title,
    date: movie.releaseDate,
  });
  return (
    <>
      <MoreActions label="More actions">
        <MovieMenuItems {...props} {...{ images, fix, unmatch, remove }} />
      </MoreActions>
      <EditImages ref={images} title={movie.title} {...movieImages(movieId, movie)} />
      <FixMatch fix={fix} unmatch={unmatch} title={toMatch} />
      <DeleteMovie ref={remove} libraryId={libraryId} movieId={movieId} movie={movie} />
    </>
  );
}
