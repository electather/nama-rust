import { TextField } from "@hub/features/account/components/text-field";
import { LinkFormEnd } from "@hub/features/links/components/link-form-end";
import { useApproveServerLink } from "@hub/features/links/data/servers";
import { linkErrors, typedCode } from "@hub/features/links/lib/link-refusal";

/** Entering the code a Server shows, which makes it a Linked Server of the signed-in Account. */
export function LinkServerForm() {
  const approve = useApproveServerLink();
  const errors = linkErrors(approve.error);
  return (
    <form
      aria-labelledby="link-server"
      className="flex flex-col gap-3"
      onSubmit={(event) => {
        event.preventDefault();
        const form = event.currentTarget;
        approve.mutate(
          { code: typedCode(form) },
          {
            onSuccess: () => {
              form.reset();
            },
          },
        );
      }}
    >
      <h2 id="link-server" className="text-lg/6 font-medium">
        Link a Server
      </h2>
      <TextField
        label="Code"
        name="code"
        autoComplete="off"
        autoCapitalize="characters"
        spellCheck={false}
        placeholder="XXXX-XXXX"
        hint="The Server shows it under Settings, Server, once its Hub Plugin is enabled."
        error={errors.code}
      />
      <LinkFormEnd
        linked={approve.data?.server?.name}
        errors={errors}
        pending={approve.isPending}
      />
    </form>
  );
}
