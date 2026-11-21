import type { PluginOrderKind } from "@/features/admin-libraries/lib/recognisers";

/** The folders typed one per line, without blank lines or stray spaces. */
export function foldersTyped(typed: string) {
  return typed
    .split("\n")
    .map((folder) => folder.trim())
    .filter((folder) => folder !== "");
}

/** The title sources the form holds, in the order the Admin set. */
export function titleSourcesTyped(typed: FormData) {
  return typed.getAll("titleSources").filter((each) => typeof each === "string");
}

/** The ids of the Plugins of `kind` the form holds, in the order the Admin set. */
export function pluginOrderTyped(typed: FormData, { name }: PluginOrderKind) {
  return typed.getAll(name).filter((each) => typeof each === "string");
}
