-- This file should undo anything in `up.sql`
ALTER TABLE series
	ALTER COLUMN name SET DATA TYPE TEXT;

ALTER TABLE author
	ALTER COLUMN name SET DATA TYPE TEXT;

DROP EXTENSION citext;
