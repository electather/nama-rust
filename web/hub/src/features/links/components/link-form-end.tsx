import { LinkRemedy } from "@hub/features/links/components/link-remedy";
import type { LinkErrors } from "@hub/features/links/lib/link-refusal";
import { Button } from "nama-ui/components/button";

/** The end of the code form: the Server just Linked or why not, and its buttons. */
export function LinkFormEnd({
  linked,
  errors,
  pending,
}: {
  /** The name of the Server just Linked, once one is. */
  linked: string | undefined;
  errors: LinkErrors;
  pending: boolean;
}) {
  return (
    <>
      <output className="text-sm empty:hidden">
        {linked === undefined ? undefined : `${linked} is Linked to your Account.`}
      </output>
      <p role="alert" className="text-sm text-destructive empty:hidden">
        {errors.form}
      </p>
      <div className="flex flex-wrap gap-2">
        <Button type="submit" disabled={pending}>
          {pending ? "Linking…" : "Link Server"}
        </Button>
        <LinkRemedy errors={errors} />
      </div>
    </>
  );
}
