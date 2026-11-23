import { Button } from "nama-ui/components/button";

import { FormField } from "@/shared/components/form-field/form-field";
import { type ToMatch, searchTyped } from "@/shared/lib/find-a-match";
import { onSubmitted } from "@/shared/lib/forms";

/** The title and year to search the title source for, starting from `item`'s. */
export function SearchForm({
  item,
  onSearch,
}: {
  item: ToMatch["item"];
  onSearch: (search: ReturnType<typeof searchTyped>) => void;
}) {
  return (
    <form
      className="flex flex-wrap items-end gap-3"
      onSubmit={onSubmitted((typed) => {
        onSearch(searchTyped(typed, item.kind));
      })}
    >
      <div className="min-w-48 flex-1">
        <FormField label="Title" name="title" autoComplete="off" defaultValue={item.title} />
      </div>
      <div className="w-36">
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
  );
}
