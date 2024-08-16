-- Your SQL goes here
CREATE TABLE wish (
	id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
	owner uuid NOT NULL REFERENCES users(id),
	name TEXT NOT NULL
);

CREATE TABLE wishauthor (
	wish uuid NOT NULL REFERENCES wish(id),
	author INT NOT NULL REFERENCES author(id),
	PRIMARY KEY (wish, author)
);

CREATE TABLE wishseries (
	wish uuid REFERENCES wish(id) PRIMARY KEY,
	series uuid NOT NULL REFERENCES series(id),
	number INT NOT NULL,
	UNIQUE (series, number)
);
