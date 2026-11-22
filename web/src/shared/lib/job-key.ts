/**
 * The id `?job=` opens a Job by in Settings' Jobs section. Plugin and Job ids hold only a-z, 0-9
 * and `-`, so it names one Job.
 */
export function jobKey(pluginId: string, jobId: string) {
  return `${pluginId}:${jobId}`;
}

/** Settings open on the Job `jobId` of the Plugin `pluginId`, from wherever Settings links to it. */
export function jobSearch(pluginId: string, jobId: string) {
  return {
    settings: "jobs" as const,
    plugin: undefined,
    user: undefined,
    setting: undefined,
    library: undefined,
    job: jobKey(pluginId, jobId),
  };
}
