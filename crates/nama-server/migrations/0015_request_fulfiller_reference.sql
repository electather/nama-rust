-- What the Fulfiller called a Request once it took it, which only that Fulfiller reads: NULL until
-- a Fulfiller has taken the Request.
ALTER TABLE requests ADD COLUMN fulfiller_reference TEXT;
