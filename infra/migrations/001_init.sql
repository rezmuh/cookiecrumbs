-- migrate:up

CREATE TABLE demo_items (
    id SERIAL PRIMARY KEY,
    service_name TEXT NOT NULL,
    message TEXT NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- migrate:down

DROP TABLE IF EXISTS demo_items;
