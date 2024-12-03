-- Your SQL goes here
ALTER TABLE users
ADD COLUMN public_ongoing bool NOT NULL DEFAULT false;
