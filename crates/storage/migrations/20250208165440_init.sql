CREATE TABLE discovered (
    protocol VARCHAR(40) PRIMARY KEY NOT NULL,
    last_block BIGINT NOT NULL
);

CREATE TABLE state (
    filter_hash BIGINT NOT NULL DEFAULT 0
);

CREATE TABLE pool (
    id SERIAL PRIMARY KEY,
    address CHAR(42) UNIQUE NOT NULL,
    protocol VARCHAR(40) NOT NULL
);

CREATE TABLE filtered_pool (
    id SERIAL PRIMARY KEY,
    address CHAR(42) UNIQUE NOT NULL,
    protocol VARCHAR(40) NOT NULL
);

INSERT INTO state (filter_hash) VALUES (0);
