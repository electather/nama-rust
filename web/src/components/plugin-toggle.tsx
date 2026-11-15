import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";
import { Ban, Power, PowerOff } from "lucide-react";
import { useId, useRef, useState } from "react";

import { ConfirmModal } from "@/components/confirm-dialog";
import { GrantedPermissions, pluginPermissions } from "@/components/plugin-at-a-glance";
import { StateLabel } from "@/components/state-label";
import { Button } from "@/components/ui/button";
import {
  type PluginPermission,
  PluginService,
  type PluginSummary,
  type Setting,
} from "@/gen/nama/v1/plugin_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
// oxlint-disable-next-line import/max-dependencies -- enabling asks which requested Plugin Permissions to grant, in a dialog
import { refusal, refusalOf } from "@/lib/forms";
import { pluginName, pluginsKey } from "@/lib/plugins";

/**
 * Whether the Plugin `summary` is enabled, a button to change that, and why the Server refused to,
 * for a wrapping flex row. A Plugin the Server refused only says so. A refusal names a setting by
 * its label in `settings`, or by its key without them, as on the list.
 */
export function PluginToggle({
  summary,
  settings,
}: {
  summary: PluginSummary;
  settings?: Setting[];
}) {
  if (summary.refusedReason !== "") return <StateLabel label="Refused" Icon={Ban} />;
  // Enabling a Plugin that requests Plugin Permissions first asks which to grant.
  if (!summary.enabled && summary.requestedPermissions.length > 0) {
    return <GrantAndEnable summary={summary} settings={settings} />;
  }
  return <EnableOrDisable summary={summary} settings={settings} />;
}

/** Refreshes the list and the details page once a change settles, refused or not, as either may be out of date. */
function useRefreshPlugins() {
  const queryClient = useQueryClient();
  return () => queryClient.invalidateQueries({ queryKey: pluginsKey });
}

function EnableOrDisable({
  summary,
  settings,
}: {
  summary: PluginSummary;
  settings: Setting[] | undefined;
}) {
  const onSettled = useRefreshPlugins();
  const enable = useMutation(PluginService.method.enablePlugin, { onSettled });
  const disable = useMutation(PluginService.method.disablePlugin, { onSettled });
  const { change, action, ...state } = summary.enabled
    ? { change: disable, action: "Disable", label: "Enabled", Icon: Power }
    : { change: enable, action: "Enable", label: "Disabled", Icon: PowerOff };
  // A fragment: the granted Permissions and the alert each take a whole line of the header, which wraps.
  return (
    <>
      <div className="flex items-center gap-2">
        <StateLabel {...state} />
        <Button
          variant="ghost"
          aria-label={`${action} ${pluginName(summary)}`}
          disabled={change.isPending}
          onClick={() => {
            change.mutate({ id: summary.id });
          }}
        >
          {action}
        </Button>
      </div>
      {summary.enabled && <GrantedPermissions granted={summary.grantedPermissions} />}
      {change.isError && (
        <p role="alert" className="w-full text-sm text-pretty text-destructive">
          {refusedChange(change.error, `${action.toLowerCase()} ${pluginName(summary)}`, settings)}
        </p>
      )}
    </>
  );
}

/**
 * A disabled Plugin that requests Plugin Permissions, with an Enable button that opens a dialog
 * with one checkbox for each, none checked, and enables it granting only those checked. The
 * dialog says why the Server refused.
 */
function GrantAndEnable({
  summary,
  settings,
}: {
  summary: PluginSummary;
  settings: Setting[] | undefined;
}) {
  const enable = useMutation(PluginService.method.enablePlugin, { onSettled: useRefreshPlugins() });
  const name = pluginName(summary);
  const dialog = useRef<HTMLDialogElement>(null);
  const [openings, setOpenings] = useState(0);
  return (
    <div className="flex items-center gap-2">
      <StateLabel label="Disabled" Icon={PowerOff} />
      <Button
        variant="ghost"
        aria-label={`Enable ${name}`}
        aria-haspopup="dialog"
        onClick={() => {
          setOpenings(openings + 1);
          dialog.current?.showModal();
        }}
      >
        Enable
      </Button>
      <ConfirmModal
        ref={dialog}
        openings={openings}
        action="Enable"
        title={`Enable ${name}?`}
        description="It asks to do these in the Server. It gets only the ones you check. To change them later, disable it and enable it again."
        onConfirm={(typed) => {
          const checked = typed.getAll("permission");
          const grantedPermissions = summary.requestedPermissions.filter((permission) =>
            checked.includes(String(permission)),
          );
          return enable.mutateAsync({ id: summary.id, grantedPermissions });
        }}
        refusal={(error) => refusedChange(error, `enable ${name}`, settings)}
      >
        <RequestedPermissions requested={summary.requestedPermissions} />
      </ConfirmModal>
    </div>
  );
}

/** A checkbox for each Plugin Permission the Plugin requests. */
function RequestedPermissions({ requested }: { requested: PluginPermission[] }) {
  return (
    <ul className="flex flex-col gap-3">
      {requested.map((permission) => (
        <li key={permission}>
          <PermissionCheckbox permission={permission} />
        </li>
      ))}
    </ul>
  );
}

/** One requested Plugin Permission, unchecked until the Admin grants it. */
function PermissionCheckbox({ permission }: { permission: PluginPermission }) {
  const id = useId();
  const { label, about } = pluginPermissions[permission] ?? {
    label: `Permission ${String(permission)}`,
    about: "This app doesn’t know it yet.",
  };
  return (
    <div className="flex gap-3">
      <input
        id={id}
        type="checkbox"
        name="permission"
        value={permission}
        aria-describedby={`${id}-about`}
        className="mt-0.5 size-4 shrink-0 accent-foreground"
      />
      <div className="text-sm">
        <label htmlFor={id} className="font-medium">
          {label}
        </label>
        <p id={`${id}-about`} className="text-muted-foreground">
          {about}
        </p>
      </div>
    </div>
  );
}

/** Why `doing`, enabling or disabling a Plugin, failed, by its Refusal (web/AGENTS.md). */
function refusedChange(error: unknown, doing: string, settings: Setting[] | undefined) {
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

/** The refused-to-start sentence, with the Plugin's own reason when it gave one. */
function refusedToStart(reason: string | undefined) {
  return reason !== undefined && reason !== ""
    ? `It refused to start: ${reason.replace(/\.$/u, "")}.`
    : "It refused to start.";
}
