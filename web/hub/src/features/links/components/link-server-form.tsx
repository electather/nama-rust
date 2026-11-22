import { CodeForm } from "@hub/features/links/components/code-form";
import { useApproveServerLink } from "@hub/features/links/data/servers";

/** Entering the code a Server shows, which makes it a Linked Server of the signed-in Account. */
export function LinkServerForm() {
  const approve = useApproveServerLink();
  const linked = approve.data?.server?.name;
  return (
    <CodeForm
      id="link-server"
      heading="Link a Server"
      hint="The Server shows it under Settings, Server, once its Hub Plugin is enabled."
      action="Link Server"
      linked={linked === undefined ? undefined : `${linked} is Linked to your Account.`}
      approve={{
        mutate: (code, done) => {
          approve.mutate({ code }, { onSuccess: done });
        },
        error: approve.error,
        isPending: approve.isPending,
      }}
    />
  );
}
