/**
 * The id of a Job's row on the Jobs page, which a Plugin's page links to as its hash. Plugin and
 * Job ids hold no `/`, so it names one Job.
 */
export function jobAnchor(pluginId: string, jobId: string) {
  return `${pluginId}/${jobId}`;
}
