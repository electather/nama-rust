import { CopyLinkButton } from "@/features/invites/components/copy-link-button";
import { ShareLinkButton } from "@/features/invites/components/share-link-button";
import type { Invite } from "@/gen/nama/v1/user_pb";
import { FormField } from "@/shared/components/form-field/form-field";
import { dayOf } from "@/shared/lib/title-facts";

/** An Invite's link, to copy or share, and until when it works. */
export function LinkToShare({ url, invite }: { url: string; invite: Invite }) {
  return (
    <div className="mt-6 flex flex-col gap-3 rounded-xl border bg-surface-raised p-4.5">
      <FormField
        label="Invite link"
        name="inviteLink"
        readOnly
        required={false}
        value={url}
        onFocus={(event) => {
          event.currentTarget.select();
        }}
        hint={`Anyone with it can create one User, once, until ${dayOf(invite.expiresAt)}.`}
      />
      {/* Ghost buttons are pulled back by their own padding, so their words line up with the field. */}
      <div className="-ms-4 flex flex-wrap gap-2">
        <CopyLinkButton url={url} />
        {"share" in navigator && <ShareLinkButton url={url} />}
      </div>
    </div>
  );
}
