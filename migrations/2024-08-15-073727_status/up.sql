-- Your SQL goes here
ALTER TABLE book
ADD COLUMN owned bool NOT NULL DEFAULT 'false',
ADD COLUMN read bool NOT NULL DEFAULT 'fals';

ALTER TABLE book
ALTER COLUMN owned DROP DEFAULT,
ALTER COLUMN owned DROP DEFAULT;
