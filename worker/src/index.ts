import { Container, getContainer } from "@cloudflare/containers";

export interface Env {
  HOMEABELL: DurableObjectNamespace<HomeabellContainer>;
  DB: D1Database;
  INTERNAL_DB_SECRET: string;
  /** Public origin the container uses to call back into this Worker for D1 RPC. */
  DB_RPC_URL: string;
  /** Optional local seed accounts (from `.dev.vars`). */
  ALLOW_DEV_SEED_USERS?: string;
  TEST_USER_EMAIL?: string;
  TEST_USER_PASSWORD?: string;
  TEST_USER_2_EMAIL?: string;
  TEST_USER_2_PASSWORD?: string;
}

type RpcBody = {
  op?: string;
  sql?: string;
  params?: unknown[];
};

function bindParams(stmt: D1PreparedStatement, params: unknown[]): D1PreparedStatement {
  if (!params.length) return stmt;
  return stmt.bind(
    ...params.map((p) => {
      if (p === null || p === undefined) return null;
      if (typeof p === "boolean") return p ? 1 : 0;
      return p as string | number | null;
    }),
  );
}

async function handleDbRpc(request: Request, env: Env): Promise<Response> {
  const auth = request.headers.get("Authorization") ?? "";
  const expected = `Bearer ${env.INTERNAL_DB_SECRET}`;
  if (!env.INTERNAL_DB_SECRET || auth !== expected) {
    return Response.json({ ok: false, error: "unauthorized" }, { status: 401 });
  }

  let body: RpcBody;
  try {
    body = (await request.json()) as RpcBody;
  } catch {
    return Response.json({ ok: false, error: "invalid JSON body" }, { status: 400 });
  }

  const op = body.op ?? "query";
  const sql = (body.sql ?? "").trim();
  const params = Array.isArray(body.params) ? body.params : [];

  if (!sql && op !== "batch_sql") {
    return Response.json({ ok: false, error: "sql is required" }, { status: 400 });
  }

  try {
    if (op === "execute") {
      const result = await bindParams(env.DB.prepare(sql), params).run();
      return Response.json({
        ok: true,
        changes: result.meta.changes ?? 0,
      });
    }

    if (op === "query") {
      const rows = await bindParams(env.DB.prepare(sql), params).raw();
      return Response.json({ ok: true, rows });
    }

    if (op === "batch_sql") {
      const statements = sql
        .split(";")
        .map((s) => s.trim())
        .filter((s) => {
          if (!s) return false;
          const withoutComments = s
            .split("\n")
            .map((line) => line.trim())
            .filter((line) => line && !line.startsWith("--"))
            .join("\n")
            .trim();
          return withoutComments.length > 0;
        });
      if (statements.length === 0) {
        return Response.json({ ok: true, changes: 0 });
      }
      await env.DB.batch(statements.map((s) => env.DB.prepare(s)));
      return Response.json({ ok: true, changes: statements.length });
    }

    return Response.json({ ok: false, error: `unknown op: ${op}` }, { status: 400 });
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    return Response.json({ ok: false, error: message }, { status: 500 });
  }
}

export class HomeabellContainer extends Container<Env> {
  defaultPort = 3000;
  sleepAfter = "30m";

  constructor(ctx: ConstructorParameters<typeof Container<Env>>[0], env: Env) {
    super(ctx, env);
    this.envVars = buildContainerEnv(env);
  }

  /**
   * Wait for port readiness with a long timeout and without tying the wait to the
   * incoming request AbortSignal (browser/client abort was causing
   * "Error checking 3000: The operation was aborted" on slow cold starts).
   */
  override async fetch(request: Request): Promise<Response> {
    try {
      await this.startAndWaitForPorts({
        ports: this.defaultPort,
        cancellationOptions: {
          portReadyTimeoutMS: 180_000,
          instanceGetTimeoutMS: 60_000,
        },
      });
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      return new Response(`Failed to start container: ${message}`, { status: 500 });
    }

    this.renewActivityTimeout();
    const container = this.ctx.container;
    if (!container) {
      return new Response("Container runtime unavailable", { status: 500 });
    }
    const tcpPort = container.getTcpPort(this.defaultPort);
    const containerUrl = request.url.replace("https:", "http:");
    return tcpPort.fetch(containerUrl, request);
  }
}

function buildContainerEnv(env: Env): Record<string, string> {
  const base: Record<string, string> = {
    HOST: "0.0.0.0",
    PORT: "3000",
    STATIC_DIR: "/app/static",
    SESSION_SAME_SITE: "Lax",
  };
  forwardSeedEnv(env, base);

  const secret = env.INTERNAL_DB_SECRET?.trim();
  if (secret) {
    return {
      ...base,
      SESSION_SECURE: "true",
      DB_MODE: "d1",
      DB_RPC_URL: env.DB_RPC_URL,
      INTERNAL_DB_SECRET: secret,
    };
  }
  // Local `wrangler dev` without secrets: SQLite inside the container.
  return {
    ...base,
    SESSION_SECURE: "false",
    DB_MODE: "local",
    DATABASE_URL: "sqlite:/tmp/mortgage.db",
  };
}

const SEED_ENV_KEYS = [
  "ALLOW_DEV_SEED_USERS",
  "TEST_USER_EMAIL",
  "TEST_USER_PASSWORD",
  "TEST_USER_2_EMAIL",
  "TEST_USER_2_PASSWORD",
] as const;

function forwardSeedEnv(env: Env, out: Record<string, string>) {
  for (const key of SEED_ENV_KEYS) {
    const value = env[key]?.trim();
    if (value) {
      out[key] = value;
    }
  }
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);
    if (url.pathname === "/_internal/db" && request.method === "POST") {
      return handleDbRpc(request, env);
    }

    return getContainer(env.HOMEABELL).fetch(request);
  },
};
