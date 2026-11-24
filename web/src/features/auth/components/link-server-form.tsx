import { LinkCodeForm } from "@/features/auth/components/link-code-form";
import { useApproveServerLink } from "@/features/auth/data/use-approve-server-link";

/** Entering the code a Server shows, which makes it a Linked Server of the signed-in Account. */
export function LinkServerForm() {
  const approve = useApproveServerLink();
  const linked = approve.data?.server?.name;
  return (
    <LinkCodeForm
      // The Server's Settings tells its Admin to enter the code under this heading.
      heading="Link a Server"
      hint="The Server shows it in Settings, under Server."
      action="Link Server"
      linked={linked === undefined ? undefined : `${linked} is Linked to your Account.`}
      pending={approve.isPending}
      error={approve.error}
      onCode={(code, form) => {
        approve.mutate(
          { code },
          {
            onSuccess: () => {
              form.reset();
            },
          },
        );
      }}
    />
  );
}
