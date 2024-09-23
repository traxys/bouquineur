-- This file should undo anything in `up.sql`
ALTER TABLE series
DROP COLUMN ongoing,
DROP COLUMN total_count;
