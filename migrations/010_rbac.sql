-- Account RBAC: Better Auth admin plugin fields + custom tier; mirror on domain users.
-- Profile sharing Owner/Editor roles are separate (profile_members).

ALTER TABLE "user" ADD COLUMN "role" text DEFAULT 'user';
ALTER TABLE "user" ADD COLUMN "banned" integer DEFAULT 0;
ALTER TABLE "user" ADD COLUMN "banReason" text;
ALTER TABLE "user" ADD COLUMN "banExpires" date;
ALTER TABLE "user" ADD COLUMN "tier" text DEFAULT 'unpaid';

ALTER TABLE "session" ADD COLUMN "impersonatedBy" text;

ALTER TABLE users ADD COLUMN role TEXT NOT NULL DEFAULT 'user';
ALTER TABLE users ADD COLUMN tier TEXT NOT NULL DEFAULT 'unpaid';
