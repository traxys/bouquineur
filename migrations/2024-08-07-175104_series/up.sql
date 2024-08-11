-- Your SQL goes here
CREATE TABLE series (
	id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
	owner uuid NOT NULL REFERENCES users(id),
	name TEXT NOT NULL,
	UNIQUE (owner, name)
);

CREATE TABLE bookseries (
	book uuid REFERENCES book(id) PRIMARY KEY,
	series uuid NOT NULL REFERENCES series(id),
	number INT NOT NULL,
	UNIQUE (series, number)
);
