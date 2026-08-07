-- Time-based paid access: paid while utcnow < paidUntil / paid_until.
-- Legacy tier columns remain unused (SQLite drop is awkward across envs).

ALTER TABLE "user" ADD COLUMN "paidUntil" text;
ALTER TABLE users ADD COLUMN paid_until TEXT;

-- Anyone previously marked paid gets a far-future entitlement.
UPDATE "user" SET "paidUntil" = '9999-12-31T23:59:59.000Z' WHERE tier = 'paid';
UPDATE users SET paid_until = '9999-12-31T23:59:59.000Z' WHERE tier = 'paid';
