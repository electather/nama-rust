import { LinkToShare } from "@/features/invites/components/link-to-share";
import type { CreateInviteResponse } from "@/gen/nama/v1/user_pb";

/** The link of the Invite just `created`, if one was, to copy or share: the Server sends nothing. */
export function InviteLink({ created }: { created: CreateInviteResponse | undefined }) {
  return created?.invite ? <LinkToShare url={created.url} invite={created.invite} /> : null;
}
