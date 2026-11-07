-- Why a Request was declined, for its Requesters to read: NULL unless the decliner gave a reason.
ALTER TABLE requests ADD COLUMN decline_reason TEXT;
