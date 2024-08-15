-- This file should undo anything in `up.sql`
ALTER TABLE book
DROP COLUMN owned,
DROP COLUMN read;
