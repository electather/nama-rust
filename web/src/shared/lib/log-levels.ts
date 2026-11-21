import { PluginLogLevel } from "@/gen/nama/v1/plugin_pb";

/** Each log level in plain words. */
export const levels: Record<PluginLogLevel, string> = {
  [PluginLogLevel.UNSPECIFIED]: "",
  [PluginLogLevel.DEBUG]: "Debug",
  [PluginLogLevel.INFO]: "Info",
  [PluginLogLevel.WARN]: "Warning",
  [PluginLogLevel.ERROR]: "Error",
};
