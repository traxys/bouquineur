-- Your SQL goes here
ALTER TABLE series
ADD COLUMN ongoing bool NOT NULL DEFAULT 'false',
ADD COLUMN total_count INT;

ALTER TABLE series
ALTER COLUMN ongoing DROP DEFAULT;
