import { useId } from "react";

import { AddSources } from "@/features/admin-libraries/components/title-sources-field/add-sources";
import { NoSourceSearches } from "@/features/admin-libraries/components/title-sources-field/no-source-searches";
import { SourceList } from "@/features/admin-libraries/components/title-sources-field/source-list";
import { useTitleSources } from "@/features/admin-libraries/data/use-title-sources";

/**
 * The Library's title sources in their `order`, which an Admin reorders, adds to and removes from,
 * each change handed to `setOrder`. At least one always stays.
 */
export function TitleSourcesField({
  order,
  setOrder,
  error,
}: {
  order: string[];
  setOrder: (order: string[]) => void;
  error?: string | undefined;
}) {
  const id = useId();
  const data = useTitleSources();
  const listed = data?.titleSources ?? [];

  return (
    <fieldset className="flex flex-col gap-2" aria-describedby={`${id}-hint ${id}-error`}>
      <legend className="mb-2 text-sm font-medium">Title sources</legend>
      <SourceList order={order} setOrder={setOrder} listed={listed} loaded={data !== undefined} />
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
