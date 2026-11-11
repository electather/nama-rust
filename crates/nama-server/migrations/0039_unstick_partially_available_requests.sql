-- Only a Request its Fulfiller has becomes Partially Available (ADR 0012). One a scan moved there
-- before it was handed off couldn't be decided, handed off or retried, so it goes back to Pending
-- for a Request manager to decide again. A Followed Show's Season is left alone, as a scan opens
-- a new one Partially Available for an Approved Followed Show; a stuck one is rare and can be
-- cancelled.
UPDATE requests SET state = 'pending'
WHERE state = 'partially_available' AND fulfiller_reference IS NULL
  AND followed_show_id IS NULL;
