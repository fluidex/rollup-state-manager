CREATE TABLE l2block (
    block_id integer PRIMARY KEY,
    new_root VARCHAR(256) NOT NULL,
    witness jsonb NOT NULL,
    created_time TIMESTAMP(0) NOT NULL DEFAULT CURRENT_TIMESTAMP
);
