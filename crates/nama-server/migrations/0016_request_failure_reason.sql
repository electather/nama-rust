-- Why the Fulfiller didn't take a Request, in its own words: NULL unless the Request is Failed.
ALTER TABLE requests ADD COLUMN failure_reason TEXT;
