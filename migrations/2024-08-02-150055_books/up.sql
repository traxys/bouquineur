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

CREATE TABLE bookAuthor (
	book uuid NOT NULL REFERENCES book(id),
	name TEXT NOT NULL,
	PRIMARY KEY (book, name)
);

CREATE TABLE bookTag (
	book uuid NOT NULL REFERENCES book(id),
	tag TEXT NOT NULL,
	PRIMARY KEY (book, tag)
);
