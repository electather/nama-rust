import { Bug, CircleX, Info, type LucideIcon, TriangleAlert } from "lucide-react";

import { PluginLogLevel } from "@/gen/nama/v1/plugin_pb";

/**
 * How each log level is drawn: its word, its icon, and the ink that makes it stand out, if it does.
 * The quietest first, as a log's filter offers them. A line of no level has none. Only Error is
 * red, DESIGN.md's one status colour, and Warning is Ink.
 */
export const levels = new Map<PluginLogLevel, { label: string; Icon: LucideIcon; tone: string }>([
  [PluginLogLevel.DEBUG, { label: "Debug", Icon: Bug, tone: "" }],
  [PluginLogLevel.INFO, { label: "Info", Icon: Info, tone: "" }],
  [PluginLogLevel.WARN, { label: "Warning", Icon: TriangleAlert, tone: "text-foreground" }],
  [PluginLogLevel.ERROR, { label: "Error", Icon: CircleX, tone: "text-destructive" }],
]);
