ALTER TABLE providers ADD COLUMN resend_interval_days INTEGER NOT NULL DEFAULT 7 CHECK (resend_interval_days IN (3, 7, 15, 30));
