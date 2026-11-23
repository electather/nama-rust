import { TextField } from "@hub/features/account/components/text-field";
import { useSendsSignInLinks } from "@hub/features/account/data/sign-in-links";
import { LinkFormEnd } from "@hub/features/links/components/link-form-end";
import { linkErrors, typedCode } from "@hub/features/links/lib/link-refusal";

/** Entering a code a Server shows, for itself or for one of its Users, which Links it. */
export function CodeForm({
  id,
  heading,
  hint,
  action,
  linked,
  approve,
}: {
  id: string;
  heading: string;
  /** Where the code shows. */
  hint: string;
  /** What the submit button says, such as “Link Server”. */
  action: string;
  /** What to say once the code Linked something. */
  linked: string | undefined;
  /** Sends the code, and what became of it. */
  approve: {
    mutate: (code: string, done: () => void) => void;
    error: unknown;
    isPending: boolean;
  };
}) {
  const errors = linkErrors(approve.error, useSendsSignInLinks());
  return (
    <form
      aria-labelledby={id}
      className="flex flex-col gap-3"
      onSubmit={(event) => {
        event.preventDefault();
        const form = event.currentTarget;
        approve.mutate(typedCode(form), () => {
          form.reset();
        });
      }}
    >
      <h2 id={id} className="text-lg/6 font-medium">
        {heading}
      </h2>
      <TextField
        label="Code"
        name="code"
        autoComplete="off"
        autoCapitalize="characters"
        spellCheck={false}
        placeholder="XXXX-XXXX"
        hint={hint}
        error={errors.code}
      />
      <LinkFormEnd action={action} linked={linked} errors={errors} pending={approve.isPending} />
    </form>
  );
}
