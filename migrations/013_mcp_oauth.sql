-- Better Auth MCP / OIDC provider tables (Worker owns writes).
CREATE TABLE IF NOT EXISTS "oauthApplication" (
    "id" text not null primary key,
    "name" text not null,
    "icon" text,
    "metadata" text,
    "clientId" text not null unique,
    "clientSecret" text,
    "redirectUrls" text not null,
    "type" text not null,
    "disabled" integer,
    "userId" text references "user" ("id") on delete cascade,
    "createdAt" date not null,
    "updatedAt" date not null
);

CREATE TABLE IF NOT EXISTS "oauthAccessToken" (
    "id" text not null primary key,
    "accessToken" text not null unique,
    "refreshToken" text not null unique,
    "accessTokenExpiresAt" date not null,
    "refreshTokenExpiresAt" date not null,
    "clientId" text not null references "oauthApplication" ("clientId") on delete cascade,
    "userId" text references "user" ("id") on delete cascade,
    "scopes" text not null,
    "createdAt" date not null,
    "updatedAt" date not null
);

CREATE TABLE IF NOT EXISTS "oauthConsent" (
    "id" text not null primary key,
    "clientId" text not null references "oauthApplication" ("clientId") on delete cascade,
    "userId" text not null references "user" ("id") on delete cascade,
    "scopes" text not null,
    "createdAt" date not null,
    "updatedAt" date not null,
    "consentGiven" integer not null
);

CREATE INDEX IF NOT EXISTS "oauthApplication_userId_idx" on "oauthApplication" ("userId");
CREATE INDEX IF NOT EXISTS "oauthAccessToken_clientId_idx" on "oauthAccessToken" ("clientId");
CREATE INDEX IF NOT EXISTS "oauthAccessToken_userId_idx" on "oauthAccessToken" ("userId");
CREATE INDEX IF NOT EXISTS "oauthConsent_clientId_idx" on "oauthConsent" ("clientId");
CREATE INDEX IF NOT EXISTS "oauthConsent_userId_idx" on "oauthConsent" ("userId");
