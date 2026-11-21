import { metadataLine } from "@/shared/lib/title-facts";

/** What a title is and its genres in one line, such as "Show · Thriller · Drama". */
export function TitleGenres({ kind, genres }: { kind: "Movie" | "Show"; genres: string[] }) {
  return <p className="text-sm text-foreground/80">{metadataLine(kind, ...genres)}</p>;
}
