import { type ComponentProps, useRef } from "react";

import { DeleteMovie } from "@/features/title/components/delete-movie";
import { EditImages } from "@/features/title/components/edit-images";
import { MoreActions } from "@/features/title/components/more-actions";
import { MovieMenuItems } from "@/features/title/components/movie-menu-items";
import { movieImages } from "@/features/title/lib/title-images";
import type { Opener } from "@/shared/lib/opener";

/**
 * A Movie's "…" menu, with the dialogs its items open beside it, so they stay once the menu
 * closes.
 */
export function MovieMoreMenu(
  props: Omit<ComponentProps<typeof MovieMenuItems>, "images" | "remove">,
) {
  const images = useRef<Opener>(null);
  const remove = useRef<Opener>(null);
  const { movie, movieId } = props;
  return (
    <>
      <MoreActions label="More actions">
        <MovieMenuItems {...props} images={images} remove={remove} />
      </MoreActions>
      <EditImages ref={images} title={movie.title} {...movieImages(movieId, movie)} />
      <DeleteMovie ref={remove} libraryId={props.libraryId} movieId={movieId} movie={movie} />
    </>
  );
}
