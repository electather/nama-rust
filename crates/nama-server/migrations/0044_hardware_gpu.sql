-- The GPU `hardware_acceleration` runs on, by its PCI address, or '' for none, and the name it was
-- last found under, which still names it once it's gone.
ALTER TABLE server_settings ADD COLUMN hardware_gpu TEXT NOT NULL DEFAULT '';
ALTER TABLE server_settings ADD COLUMN hardware_gpu_name TEXT NOT NULL DEFAULT '';
