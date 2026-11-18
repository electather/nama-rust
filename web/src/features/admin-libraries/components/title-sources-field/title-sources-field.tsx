import { useId } from "react";

import { AddSources } from "@/features/admin-libraries/components/title-sources-field/add-sources";
import { NoSourceSearches } from "@/features/admin-libraries/components/title-sources-field/no-source-searches";
import { SourceRow } from "@/features/admin-libraries/components/title-sources-field/source-row";
import { useTitleSources } from "@/features/admin-libraries/data/use-title-sources";
import { useOrderResetWithForm } from "@/features/admin-libraries/hooks/use-order-reset-with-form";
import { sourceName, unusable } from "@/features/admin-libraries/lib/title-sources";

/** A new Library's title sources, as the Server would set them. */
const tmdbAlone = ["tmdb"];

/**
 * The Library's title sources in order, which an Admin reorders, adds to and removes from, sent as
 * one `titleSources` form value per source. At least one always stays.
 */
export function TitleSourcesField({
  initial = tmdbAlone,
  error,
}: {
  initial?: string[] | undefined;
  error?: string | undefined;
}) {
  const id = useId();
  const [order, setOrder, fieldset] = useOrderResetWithForm(initial);
  const data = useTitleSources();
  const listed = data?.titleSources ?? [];

  return (
    <fieldset
      ref={fieldset}
      className="flex flex-col gap-2"
      aria-describedby={`${id}-hint ${id}-error`}
    >
      <legend className="mb-2 text-sm font-medium">Title sources</legend>
      <ol aria-label="Title sources" className="flex flex-col gap-1">
        {order.map((namespace, index) => {
          const source = listed.find((each) => each.namespace === namespace);
          return (
            <SourceRow
              key={namespace}
              order={order}
              index={index}
              setOrder={setOrder}
              label={source === undefined ? namespace : sourceName(source)}
              // Until the list arrives, no stored source is known to be missing.
              note={data === undefined ? undefined : unusable(source)}
              artworkOnly={source?.searches === false}
            />
          );
        })}
      </ol>
      <AddSources listed={listed} order={order} setOrder={setOrder} />
      <p id={`${id}-hint`} className="text-sm text-muted-foreground">
        Titles are Matched with the first source that finds them. Artwork comes from every source in
        the order.
      </p>
      {data !== undefined && <NoSourceSearches order={order} listed={listed} />}
      {/* Always there, so screen readers announce an error as it appears. */}
      <p id={`${id}-error`} role="alert" className="text-sm text-destructive empty:hidden">
        {error}
      </p>
    </fieldset>
  );
}
