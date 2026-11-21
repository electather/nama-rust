import { useNavigate } from "@tanstack/react-router";

import type { Searchable } from "@/features/requests/lib/searchable";
import { FormField } from "@/shared/components/form-field/form-field";
import { formText, onSubmitted } from "@/shared/lib/forms";
import { Button } from "@/shared/ui/button";

/**
 * Searches this Server and the title source for the titles `kinds` names to Request, through the
 * Requests page's `?title=`.
 */
export function FindATitle({ title, kinds }: { title: string | undefined; kinds: Searchable }) {
  const navigate = useNavigate();
  const wanted = [kinds.movies && "Movie", kinds.shows && "Show"].filter(Boolean).join(" or ");
  return (
    <form
      // Keyed by the search, so going back to an earlier one shows what it searched for.
      key={title}
      className="mt-8 flex max-w-xl items-end gap-3"
      onSubmit={onSubmitted((typed) => {
        const typedTitle = formText(typed, "title").trim();
        void navigate({ to: "/requests", search: typedTitle === "" ? {} : { title: typedTitle } });
      })}
    >
      <div className="flex-1">
        <FormField
          label={`Find a ${wanted}`}
          name="title"
          type="search"
          autoComplete="off"
          required={false}
          defaultValue={title}
        />
      </div>
      <Button type="submit">Search</Button>
    </form>
  );
}
