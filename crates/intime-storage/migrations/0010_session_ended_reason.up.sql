-- Why a session closed (idle / gap / context_change / finish / close).
ALTER TABLE session ADD COLUMN ended_reason TEXT;
