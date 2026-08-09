-- Replace legacy MCP plugin tables (oauthApplication) with @better-auth/oauth-provider schema.
-- Safe when no production OAuth clients are registered yet (dynamic re-registration on connect).

DROP TABLE IF EXISTS "oauthAccessToken";
DROP TABLE IF EXISTS "oauthConsent";
DROP TABLE IF EXISTS "oauthApplication";

CREATE TABLE IF NOT EXISTS "jwks" (
    "id" text not null primary key,
    "publicKey" text not null,
    "privateKey" text not null,
    "createdAt" date not null,
    "expiresAt" date
);

CREATE TABLE IF NOT EXISTS "oauthClient" (
    "id" text not null primary key,
    "clientId" text not null unique,
    "clientSecret" text,
    "disabled" integer,
    "skipConsent" integer,
    "enableEndSession" integer,
    "subjectType" text,
    "scopes" text,
    "userId" text references "user" ("id") on delete cascade,
    "createdAt" date,
    "updatedAt" date,
    "name" text,
    "uri" text,
    "icon" text,
    "contacts" text,
    "tos" text,
    "policy" text,
    "softwareId" text,
    "softwareVersion" text,
    "softwareStatement" text,
    "redirectUris" text not null,
    "postLogoutRedirectUris" text,
    "tokenEndpointAuthMethod" text,
    "grantTypes" text,
    "responseTypes" text,
    "public" integer,
    "type" text,
    "requirePKCE" integer,
    "referenceId" text,
    "metadata" text
);

CREATE TABLE IF NOT EXISTS "oauthRefreshToken" (
    "id" text not null primary key,
    "token" text not null unique,
    "clientId" text not null references "oauthClient" ("clientId") on delete cascade,
    "sessionId" text references "session" ("id") on delete set null,
    "userId" text not null references "user" ("id") on delete cascade,
    "referenceId" text,
    "expiresAt" date not null,
    "createdAt" date not null,
    "revoked" date,
    "authTime" date,
    "scopes" text not null
);

CREATE TABLE IF NOT EXISTS "oauthAccessToken" (
    "id" text not null primary key,
    "token" text not null unique,
    "clientId" text not null references "oauthClient" ("clientId") on delete cascade,
    "sessionId" text references "session" ("id") on delete set null,
    "userId" text references "user" ("id") on delete cascade,
    "referenceId" text,
    "refreshId" text references "oauthRefreshToken" ("id") on delete cascade,
    "expiresAt" date not null,
    "createdAt" date not null,
    "scopes" text not null
);

CREATE TABLE IF NOT EXISTS "oauthConsent" (
    "id" text not null primary key,
    "clientId" text not null references "oauthClient" ("clientId") on delete cascade,
    "userId" text references "user" ("id") on delete cascade,
    "referenceId" text,
    "scopes" text not null,
    "createdAt" date not null,
    "updatedAt" date not null
);

CREATE INDEX IF NOT EXISTS "oauthClient_userId_idx" on "oauthClient" ("userId");
CREATE INDEX IF NOT EXISTS "oauthRefreshToken_clientId_idx" on "oauthRefreshToken" ("clientId");
CREATE INDEX IF NOT EXISTS "oauthRefreshToken_sessionId_idx" on "oauthRefreshToken" ("sessionId");
CREATE INDEX IF NOT EXISTS "oauthRefreshToken_userId_idx" on "oauthRefreshToken" ("userId");
CREATE INDEX IF NOT EXISTS "oauthAccessToken_clientId_idx" on "oauthAccessToken" ("clientId");
CREATE INDEX IF NOT EXISTS "oauthAccessToken_sessionId_idx" on "oauthAccessToken" ("sessionId");
CREATE INDEX IF NOT EXISTS "oauthAccessToken_userId_idx" on "oauthAccessToken" ("userId");
CREATE INDEX IF NOT EXISTS "oauthAccessToken_refreshId_idx" on "oauthAccessToken" ("refreshId");
CREATE INDEX IF NOT EXISTS "oauthConsent_clientId_idx" on "oauthConsent" ("clientId");
CREATE INDEX IF NOT EXISTS "oauthConsent_userId_idx" on "oauthConsent" ("userId");
