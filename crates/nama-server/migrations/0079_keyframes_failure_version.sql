-- The `PROBE_FAILURE_VERSION` at which listing a Version's keyframes failed or timed out, or NULL
-- if it hasn't. It goes with its row, as `keyframes_ms` does, so a file that changes and is
-- probed again has them listed again, and listing isn't tried again until then or a bump (#850).
ALTER TABLE version_probes ADD COLUMN keyframes_failure_version INTEGER;
