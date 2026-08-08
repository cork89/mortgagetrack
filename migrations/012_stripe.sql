-- Stripe customer binding + subscription snapshot (Worker sync writes these).
-- App entitlement remains Better Auth user.paidUntil / users.paid_until.

ALTER TABLE "user" ADD COLUMN stripeCustomerId text;

CREATE UNIQUE INDEX IF NOT EXISTS user_stripe_customer_id_uidx
  ON "user" (stripeCustomerId)
  WHERE stripeCustomerId IS NOT NULL;

CREATE TABLE IF NOT EXISTS stripe_subscription (
  customer_id TEXT PRIMARY KEY NOT NULL,
  user_id TEXT NOT NULL,
  status TEXT NOT NULL,
  subscription_id TEXT,
  price_id TEXT,
  current_period_start INTEGER,
  current_period_end INTEGER,
  cancel_at_period_end INTEGER NOT NULL DEFAULT 0,
  payment_method_brand TEXT,
  payment_method_last4 TEXT,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS stripe_subscription_user_id_idx
  ON stripe_subscription (user_id);
