import { LinkRemedy } from "@hub/features/links/components/link-remedy";
import type { LinkErrors } from "@hub/features/links/lib/link-refusal";
import { Button } from "nama-ui/components/button";

/** The end of a code form: what it just Linked or why not, and its buttons. */
export function LinkFormEnd({
  action,
  linked,
  errors,
  pending,
}: {
  /** What the submit button says, such as “Link Server”. */
  action: string;
  /** What to say once the code Linked something. */
  linked: string | undefined;
  errors: LinkErrors;
  pending: boolean;
}) {
  return (
    <>
      <output className="text-sm empty:hidden">{linked}</output>
      <p role="alert" className="text-sm text-destructive empty:hidden">
        {errors.form}
      </p>
      <div className="flex flex-wrap gap-2">
        <Button type="submit" disabled={pending}>
          {pending ? "Linking…" : action}
        </Button>
        <LinkRemedy errors={errors} />
      </div>
    </>
  );
}
