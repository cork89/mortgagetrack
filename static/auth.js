// Better Auth client helpers for SSR auth forms (used when data-auth-edge is set).
(() => {
  const AUTH_BASE = "/api/auth";

  function errorBox(form) {
    return form.querySelector("#auth-error, #password-status, #delete-status");
  }

  function showError(form, message) {
    const box = errorBox(form);
    if (!box) {
      alert(message);
      return;
    }
    box.innerHTML = `<p class="error" role="alert">${escapeHtml(message)}</p>`;
  }

  function escapeHtml(s) {
    return String(s)
      .replaceAll("&", "&amp;")
      .replaceAll("<", "&lt;")
      .replaceAll(">", "&gt;")
      .replaceAll('"', "&quot;");
  }

  async function authFetch(path, body) {
    const res = await fetch(`${AUTH_BASE}${path}`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      credentials: "include",
      body: JSON.stringify(body),
    });
    let data = null;
    try {
      data = await res.json();
    } catch {
      data = null;
    }
    if (!res.ok) {
      const message =
        data?.message || data?.error?.message || data?.error || `Request failed (${res.status})`;
      throw new Error(typeof message === "string" ? message : "Request failed");
    }
    return data;
  }

  function nextFromForm(form) {
    const input = form.querySelector('input[name="next"]');
    const next = input?.value?.trim();
    if (next && next.startsWith("/") && !next.startsWith("//")) return next;
    return "/";
  }

  /** MCP/OIDC authorize return: Better Auth redirects to /login?client_id=… */
  function oauthAuthorizeReturn() {
    const params = new URLSearchParams(window.location.search);
    if (!params.get("client_id")) return null;
    return `/api/auth/oauth2/authorize?${params.toString()}`;
  }

  function postAuthDestination(form) {
    return oauthAuthorizeReturn() || nextFromForm(form);
  }

  async function onLogin(form) {
    const email = form.email.value.trim();
    const password = form.password.value;
    const dest = postAuthDestination(form);
    // Relative callback paths stay same-origin; avoids Origin/trustedOrigins mismatches.
    await authFetch("/sign-in/email", { email, password, callbackURL: dest });
    window.location.assign(dest);
  }

  async function onRegister(form) {
    const email = form.email.value.trim();
    const password = form.password.value;
    const confirm = form.confirm_password.value;
    if (password !== confirm) throw new Error("Passwords do not match.");
    const name = email.includes("@") ? email.split("@")[0] : email;
    const dest = postAuthDestination(form);
    await authFetch("/sign-up/email", {
      name,
      email,
      password,
      callbackURL: dest,
    });
    window.location.assign(dest);
  }

  async function onForgot(form) {
    const email = form.email.value.trim();
    await authFetch("/request-password-reset", {
      email,
      redirectTo: `${window.location.origin}/reset-password`,
    });
    window.location.assign("/forgot-password?sent=1");
  }

  async function onReset(form) {
    const token = form.token.value;
    const password = form.password.value;
    const confirm = form.confirm_password.value;
    if (password !== confirm) throw new Error("Passwords do not match.");
    await authFetch("/reset-password", { newPassword: password, token });
    window.location.assign("/login");
  }

  async function onChangePassword(form) {
    const currentPassword = form.current_password.value;
    const newPassword = form.new_password.value;
    const confirm = form.confirm_password.value;
    if (newPassword !== confirm) throw new Error("Passwords do not match.");
    await authFetch("/change-password", {
      currentPassword,
      newPassword,
      revokeOtherSessions: true,
    });
    window.location.assign("/settings?password=updated");
  }

  async function onDeleteAccount(form) {
    const password = form.password.value;
    await authFetch("/delete-user", { password });
    window.location.assign("/");
  }

  async function onLogout(form) {
    try {
      await authFetch("/sign-out", {});
    } catch {
      // Still clear the local tower-sessions CSRF cookie via the form action.
    }
    // Fall through: allow native POST /logout to purge tower session, or hard navigate.
    if (form.dataset.skipNative === "1") {
      window.location.assign("/");
      return;
    }
    form.submit();
  }

  function wireForm(form, handler) {
    form.addEventListener("submit", async (e) => {
      e.preventDefault();
      const buttons = form.querySelectorAll('button[type="submit"], input[type="submit"]');
      buttons.forEach((b) => {
        b.disabled = true;
        b.classList.add("is-loading");
      });
      try {
        await handler(form);
      } catch (err) {
        showError(form, err instanceof Error ? err.message : String(err));
      } finally {
        buttons.forEach((b) => {
          b.disabled = false;
          b.classList.remove("is-loading");
        });
      }
    });
  }

  document.addEventListener("DOMContentLoaded", () => {
    document.querySelectorAll("form[data-auth-edge]").forEach((form) => {
      const kind = form.dataset.authEdge;
      if (kind === "login") wireForm(form, onLogin);
      else if (kind === "register") wireForm(form, onRegister);
      else if (kind === "forgot") wireForm(form, onForgot);
      else if (kind === "reset") wireForm(form, onReset);
      else if (kind === "change-password") wireForm(form, onChangePassword);
      else if (kind === "delete") wireForm(form, onDeleteAccount);
      else if (kind === "logout") wireForm(form, onLogout);
    });
  });
})();
