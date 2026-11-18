import type { Setting } from "@/gen/nama/v1/plugin_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { refusal, refusalOf } from "@/shared/lib/forms";

/** The refused-to-start sentence, with the Plugin's own reason when it gave one. */
function refusedToStart(reason: string | undefined) {
  return reason !== undefined && reason !== ""
    ? `It refused to start: ${reason.replace(/\.$/u, "")}.`
    : "It refused to start.";
}

/** Why `doing`, enabling or disabling a Plugin, failed, by its Refusal (`web/AGENTS.md`). */
export function refusedChange(error: unknown, doing: string, settings: Setting[] | undefined) {
  const { field, params } = refusalOf(error);
  // The Server names a setting as `settings.<key>`.
  const key = field.replace(/^settings\./u, "");
  const label = settings?.find((setting) => setting.key === key)?.label ?? key;
  const { fulfillerPluginId, titleKind, reason } = params;
  return refusal(
    error,
    doing,
    {},
    {
      [RefusalReason.LAST_AUTHENTICATOR]: {
        form: "It can’t be disabled while no other enabled Plugin lets Users sign in.",
      },
      [RefusalReason.FIELD_REQUIRED]: { form: `It needs ${label} set before it can be enabled.` },
      [RefusalReason.FULFILLER_TAKEN]: {
        form: `It can’t be enabled while ${fulfillerPluginId} is the ${titleKind} Fulfiller. Disable ${fulfillerPluginId} first.`,
      },
      [RefusalReason.SETTING_WRONG_KIND]: {
        form: `Its ${label} setting isn’t valid, so it can’t be enabled.`,
      },
      [RefusalReason.PLUGIN_REFUSED_TO_START]: { form: refusedToStart(reason) },
    },
  ).form;
}
