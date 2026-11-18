import { InviteLink } from "@/features/invites/components/invite-link";
import { useCreateInvite } from "@/features/invites/data/use-create-invite";
import { createInviteErrors } from "@/features/invites/lib/invite-refusals";
import type { User } from "@/gen/nama/v1/sign_in_pb";
import { FormField } from "@/shared/components/form-field/form-field";
import { PermissionsForm } from "@/shared/components/permissions-form/permissions-form";
import { useDefaultPermissions } from "@/shared/hooks/use-default-permissions";
import { formText } from "@/shared/lib/forms";
import { permissionsToInvite } from "@/shared/lib/permissions";

/** The form that makes an Invite, starting from the default Permissions `you` may pass on. */
export function CreateInvite({ you }: { you: User | undefined }) {
  const defaults = useDefaultPermissions();
  const create = useCreateInvite();
  const offer = permissionsToInvite(you);
  const errors = createInviteErrors(create.error);
  return (
    <div className="mt-4">
      <PermissionsForm
        initial={defaults.filter((held) => offer.some((choice) => choice.permission === held))}
        offer={offer}
        fields={
          <FormField
            label="Who it’s for"
            name="note"
            required={false}
            maxLength={64}
            autoComplete="off"
            hint="Optional, such as “Mum”. They pick their own name when they open the link."
            error={errors.note}
          />
        }
        onSave={(permissions, typed) => {
          create.mutate({ note: formText(typed, "note"), permissions });
        }}
        end={{
          action: create.isPending ? "Making Invite…" : "Make Invite",
          pending: create.isPending,
          done: create.isSuccess ? "Invite made. Send them this link." : undefined,
          refusal: errors.form,
        }}
      />
      <InviteLink created={create.data} />
    </div>
  );
}
