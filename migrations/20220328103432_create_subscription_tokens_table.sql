CREATE TABLE subscription_tokens(
    subscription_token TEXT NOT NULL,
    subscription_id uuid NOT NULL
        REFERENCES subscriptions (id),
    PRIMARY KEY (subscription_token)
);