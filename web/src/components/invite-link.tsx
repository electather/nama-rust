import { Copy, Share } from "lucide-react";
import { useState } from "react";

import { FormField } from "@/components/form-field";
import { Button } from "@/components/ui/button";
import type { CreateInviteResponse, Invite } from "@/gen/nama/v1/user_pb";
import { dayOf } from "@/lib/browse";

/** The link of the Invite just `created`, if one was, to copy or share: the Server sends nothing. */
export function InviteLink({ created }: { created: CreateInviteResponse | undefined }) {
  return created?.invite ? <LinkToShare url={created.url} invite={created.invite} /> : null;
}

function LinkToShare({ url, invite }: { url: string; invite: Invite }) {
  const [copied, setCopied] = useState(false);
  async function copy() {
    await navigator.clipboard.writeText(url);
    setCopied(true);
  }
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
        <Button
          variant="ghost"
          onClick={() => {
            // Unable to copy, the link can still be selected in the field.
            copy().catch(() => {});
          }}
        >
          <Copy aria-hidden className="size-4" />
          {copied ? "Copied" : "Copy link"}
        </Button>
        {"share" in navigator && (
          <Button
            variant="ghost"
            onClick={() => {
              // Dismissing the share sheet rejects, which is no failure.
              navigator.share({ url }).catch(() => {});
            }}
          >
            <Share aria-hidden className="size-4" />
            Share…
          </Button>
        )}
      </div>
    </div>
  );
}
