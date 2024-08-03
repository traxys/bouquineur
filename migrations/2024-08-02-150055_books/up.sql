-- Your SQL goes here
CREATE TABLE users (
	id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
	name TEXT NOT NULL UNIQUE
);

CREATE TABLE book (
	id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
	owner uuid NOT NULL REFERENCES users(id),
	isbn VARCHAR(17) NOT NULL,
	title TEXT NOT NULL,
	summary TEXT NOT NULL,
-- Non required information
	published DATE,
	publisher TEXT,
	language TEXT,
	googleId TEXT,
	goodreadsId TEXT,
	amazonId TEXT,
	librarythingId TEXT,
	pageCount INT,
	UNIQUE (owner, isbn)
);

CREATE TABLE author (
	id SERIAL PRIMARY KEY,
	name TEXT NOT NULL UNIQUE
);

CREATE TABLE bookAuthor (
	book uuid NOT NULL REFERENCES book(id),
	author INT NOT NULL REFERENCES author(id),
	PRIMARY KEY (book, author)
);

CREATE TABLE tag (
	id SERIAL PRIMARY KEY,
	name TEXT NOT NULL UNIQUE
);

CREATE TABLE bookTag (
	book uuid NOT NULL REFERENCES book(id),
	tag INT NOT NULL REFERENCES tag(id),
	PRIMARY KEY (book, tag)
);
