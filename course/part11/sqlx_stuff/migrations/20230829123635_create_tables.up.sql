-- Add up migration script here
CREATE TABLE users (
   user_id UUID NOT NULL UNIQUE DEFAULT gen_random_uuid(),
   name TEXT NOT NULL,
   age INT NOT NULL
)