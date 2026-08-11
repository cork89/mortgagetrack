import { byId, csrfToken, dashboard } from "./core";
import type {
  ApiResponse,
  CopyProfileData,
  DeleteProfileData,
  ImprovementPopoverMode,
  LoanFormMode,
  OpenProfileManagerOptions,
  ProfileOption,
} from "./types";

export function popover(): HTMLElement | null {
  return byId("loanPopover");
}

function improvementPopover(): HTMLElement | null {
  return byId("improvementPopover");
}

function parseJsonAttr(el: Element, name: string): string {
  try {
    const parsed: unknown = JSON.parse(el.getAttribute(name) || '""');
    return typeof parsed === "string" ? parsed : "";
  } catch {
    return "";
  }
}

export function openImprovementPopover(
  btn: HTMLElement,
  mode: ImprovementPopoverMode = "edit",
): void {
  const dash = dashboard();
  const profileId = dash?.dataset.profileId;
  if (!profileId) return;

  const form = byId<HTMLFormElement>("improvementForm");
  if (!form) return;
  const title = byId("improvementPopoverTitle");
  const isAdd = mode === "add";
  let action: string;
  if (isAdd) {
    action = `/profiles/${profileId}/improvements`;
  } else {
    const improvementId = btn.dataset.id;
    if (!improvementId) return;
    action = `/profiles/${profileId}/improvements/${improvementId}/update`;
  }

  form.action = action;
  form.setAttribute("hx-post", action);
  form.setAttribute("hx-target", "#panel-improvements");
  form.setAttribute("hx-swap", "innerHTML show:none");

  if (title) title.textContent = isAdd ? "Add" : "Edit";
  const dateEl = byId<HTMLInputElement>("improvementDate");
  const amountEl = byId<HTMLInputElement>("improvementAmount");
  if (dateEl) dateEl.value = btn.dataset.date || "";
  if (amountEl) amountEl.value = isAdd ? "" : btn.dataset.amount || "";

  const noteEl = byId<HTMLTextAreaElement>("improvementNote");
  const detailEl = byId<HTMLTextAreaElement>("improvementDetail");
  const note = isAdd ? "" : parseJsonAttr(btn, "data-note-json");
  const detail = isAdd ? "" : parseJsonAttr(btn, "data-detail-json");
  if (noteEl) {
    noteEl.value = note;
    noteEl.setCustomValidity(
      note.length > 200
        ? "Improvement notes are limited to 200 characters."
        : "",
    );
  }
  if (detailEl) {
    detailEl.value = detail;
    detailEl.setCustomValidity(
      detail.length > 1000
        ? "Improvement details are limited to 1000 characters."
        : "",
    );
  }

  if (typeof htmx !== "undefined") htmx.process(form);
  improvementPopover()?.showPopover();
  byId(isAdd ? "improvementDate" : "improvementNote")?.focus();
}

export function closeImprovementPopover(): void {
  improvementPopover()?.hidePopover();
}

function loanFormAction(mode: LoanFormMode, profileId?: string): string {
  if (mode === "edit" && profileId) return `/profiles/${profileId}`;
  return "/profiles";
}

export function setLoanFormMode(mode: LoanFormMode, profileId?: string): void {
  const form = byId<HTMLFormElement>("loanForm");
  const btn = byId<HTMLButtonElement>("buildBtn");
  if (!form) return;
  const action = loanFormAction(mode, profileId);
  form.dataset.mode = mode;
  if (profileId) form.dataset.profileId = profileId;
  else delete form.dataset.profileId;
  form.method = "post";
  form.setAttribute("action", action);
  form.action = action;
  form.setAttribute("hx-post", action);
  form.setAttribute("hx-target", "#error");
  form.setAttribute("hx-swap", "outerHTML");
  if (btn) {
    btn.setAttribute("formaction", action);
    btn.setAttribute("formmethod", "post");
  }
  if (typeof htmx !== "undefined") htmx.process(form);
}

function nextProfileName(): string {
  const select = byId<HTMLSelectElement>("profileSelect");
  const count = [...(select?.options || [])].filter((o) => o.value).length;
  return `Profile ${count + 1}`;
}

function profileSelectOptions(): HTMLOptionElement[] {
  const select = byId<HTMLSelectElement>("profileSelect");
  return [...(select?.options || [])].filter((o) => o.value);
}

export function profileOptionById(
  profileId: string,
): HTMLOptionElement | null {
  return profileSelectOptions().find((o) => o.value === profileId) || null;
}

export function isOptionOwner(
  opt: HTMLOptionElement | null | undefined,
): boolean {
  if (opt?.dataset.shared != null) return opt.dataset.shared !== "true";
  const bar = byId("profileBar");
  return bar?.dataset.isOwner !== "false";
}

function isActiveOwner(): boolean {
  const select = byId<HTMLSelectElement>("profileSelect");
  return isOptionOwner(select?.selectedOptions?.[0]);
}

export function syncOwnerState(): void {
  const owner = isActiveOwner();
  const bar = byId("profileBar");
  if (bar) bar.dataset.isOwner = owner ? "true" : "false";
}

function hideShareEditor(): void {
  const wrap = byId("shareEditor");
  const panel = byId("sharePanel");
  wrap?.classList.add("hidden");
  if (panel) panel.innerHTML = "";
}

export function fillShareInviteUrl(): void {
  const input = byId<HTMLInputElement>("shareInviteUrl");
  if (!input) return;
  const path = input.dataset.path || "";
  if (path) input.value = `${window.location.origin}${path}`;
}

function loadSharePanel(profileId: string): void {
  const wrap = byId("shareEditor");
  const panel = byId("sharePanel");
  if (!wrap || !panel || !profileId) return;
  wrap.classList.remove("hidden");
  if (typeof htmx === "undefined") return;
  htmx.ajax("GET", `/profiles/${profileId}/share-panel`, {
    target: "#sharePanel",
    swap: "innerHTML",
  });
}

function syncOpenProfileButton(profileId: string | null): void {
  const openBtn = byId<HTMLButtonElement>("openProfileBtn");
  if (!openBtn) return;
  const select = byId<HTMLSelectElement>("profileSelect");
  const activeId = select?.value || dashboard()?.dataset.profileId || "";
  const canOpen = Boolean(profileId) && profileId !== activeId;
  openBtn.classList.toggle("hidden", !canOpen);
  openBtn.disabled = !canOpen;
}

function highlightGutterSelection(profileId: string | null): void {
  const list = byId("profileGutterList");
  if (!list) return;
  list.querySelectorAll<HTMLElement>(".profile-gutter-item").forEach((btn) => {
    const on = profileId != null && btn.dataset.profileId === profileId;
    btn.classList.toggle("active", on);
    btn.setAttribute("aria-current", on ? "true" : "false");
  });
  const newBtn = byId("profileGutterNewBtn");
  newBtn?.classList.toggle("active", profileId == null);
}

export function renderProfileGutter(): void {
  const list = byId("profileGutterList");
  if (!list) return;
  const options = profileSelectOptions();
  list.replaceChildren(
    ...options.map((opt) => {
      const li = document.createElement("li");
      const btn = document.createElement("button");
      btn.type = "button";
      btn.className = "profile-gutter-item";
      btn.dataset.profileId = opt.value;
      btn.dataset.shared = opt.dataset.shared || "false";
      const name =
        opt.dataset.name ||
        opt.textContent?.replace(/\s*\(shared\)\s*$/, "") ||
        "Profile";
      const label = document.createElement("span");
      label.className = "profile-gutter-label";
      label.textContent = name;
      btn.appendChild(label);
      if (opt.dataset.shared === "true") {
        const badge = document.createElement("span");
        badge.className = "profile-gutter-badge";
        badge.textContent = "shared";
        btn.appendChild(badge);
      }
      li.appendChild(btn);
      return li;
    }),
  );
}

export function setCanCreateProfile(allowed: boolean): void {
  const actions = document.querySelector<HTMLElement>(".profile-gutter-actions");
  if (actions) actions.dataset.canCreate = allowed ? "true" : "false";
  const newBtn = byId<HTMLButtonElement>("profileGutterNewBtn");
  if (newBtn) {
    newBtn.disabled = !allowed;
    newBtn.title = allowed ? "New profile" : "Pro feature";
  }
  const copyBtn = byId<HTMLButtonElement>("copyProfileBtn");
  if (copyBtn && !copyBtn.classList.contains("hidden")) {
    copyBtn.disabled = !allowed;
    copyBtn.title = allowed ? "Copy profile" : "Pro feature";
  }
}

export function canCreateProfile(): boolean {
  const actions = document.querySelector<HTMLElement>(".profile-gutter-actions");
  if (actions?.dataset.canCreate != null) {
    return actions.dataset.canCreate === "true";
  }
  return !byId<HTMLButtonElement>("profileGutterNewBtn")?.disabled;
}

export function syncProfileSelect(
  profiles: ProfileOption[],
  selectedId: string,
): void {
  const select = byId<HTMLSelectElement>("profileSelect");
  const bar = byId("profileBar");
  if (!select) return;
  const previous = select.value;
  select.replaceChildren(
    ...(profiles.length
      ? profiles.map((p) => {
          const opt = document.createElement("option");
          opt.value = p.id;
          opt.dataset.shared = p.is_shared ? "true" : "false";
          opt.dataset.name = p.name || "";
          opt.dataset.principal = String(p.principal ?? "");
          opt.dataset.rate = String(p.rate ?? "");
          opt.dataset.term = String(p.term_years ?? "");
          opt.dataset.start = p.start_date || "";
          opt.dataset.autoMarkDue = p.auto_mark_due_paid ? "true" : "false";
          opt.textContent = p.is_shared ? `${p.name} (shared)` : p.name;
          return opt;
        })
      : [
          (() => {
            const opt = document.createElement("option");
            opt.value = "";
            opt.textContent = "No profiles yet";
            return opt;
          })(),
        ]),
  );
  const nextId =
    selectedId && profiles.some((p) => p.id === selectedId)
      ? selectedId
      : previous && profiles.some((p) => p.id === previous)
        ? previous
        : profiles[0]?.id || "";
  select.value = nextId;
  select.disabled = profiles.length === 0;
  bar?.classList.toggle("hidden", profiles.length === 0);
  syncOwnerState();
}

export function upsertProfileOption(profile: ProfileOption): void {
  const select = byId<HTMLSelectElement>("profileSelect");
  if (!select || !profile?.id) return;
  const existing = [...select.options].find((o) => o.value === profile.id);
  const opt = existing || document.createElement("option");
  opt.value = profile.id;
  opt.dataset.shared = profile.is_shared ? "true" : "false";
  opt.dataset.name = profile.name || "";
  opt.dataset.principal = String(profile.principal ?? "");
  opt.dataset.rate = String(profile.rate ?? "");
  opt.dataset.term = String(profile.term_years ?? "");
  opt.dataset.start = profile.start_date || "";
  opt.dataset.autoMarkDue = profile.auto_mark_due_paid ? "true" : "false";
  opt.textContent = profile.is_shared
    ? `${profile.name} (shared)`
    : profile.name;
  if (!existing) {
    if (select.options.length === 1 && !select.options[0]?.value) {
      select.replaceChildren(opt);
    } else {
      select.appendChild(opt);
    }
  }
  select.disabled = false;
  byId("profileBar")?.classList.remove("hidden");
}

export function refreshDashboardForActive(activeId: string): void {
  const select = byId<HTMLSelectElement>("profileSelect");
  if (!select) return;
  if (activeId) {
    if (select.value !== activeId) select.value = activeId;
    if (typeof htmx !== "undefined") htmx.trigger(select, "change");
    else select.dispatchEvent(new Event("change", { bubbles: true }));
    return;
  }
  window.location.href = "/";
}

export async function postProfileJson<T>(url: string): Promise<T> {
  const headers: Record<string, string> = {
    Accept: "application/json",
    "HX-Request": "true",
  };
  const token = csrfToken();
  if (token) headers["X-CSRF-Token"] = token;
  const res = await fetch(url, {
    method: "POST",
    headers,
    credentials: "same-origin",
  });
  let body: ApiResponse<T> | null = null;
  try {
    body = (await res.json()) as ApiResponse<T>;
  } catch {
    body = null;
  }
  if (!res.ok || !body || !("ok" in body) || !body.ok) {
    const message =
      body && "error" in body && typeof body.error === "string"
        ? body.error
        : "Request failed.";
    throw new Error(message);
  }
  return body.data;
}

export function showProfileManagerError(message: string): void {
  const err = byId("error");
  if (err) err.textContent = message || "";
}

function syncDeleteProfileButton(visible: boolean): void {
  byId("deleteProfileBtn")?.classList.toggle("hidden", !visible);
}

function syncCopyProfileButton(visible: boolean): void {
  const btn = byId<HTMLButtonElement>("copyProfileBtn");
  if (!btn) return;
  btn.classList.toggle("hidden", !visible);
  const allowed = canCreateProfile();
  btn.disabled = !allowed;
  btn.title = allowed ? "Copy profile" : "Pro feature";
}

export function selectCreateMode(): void {
  setLoanFormMode("create");
  const buildBtn = byId("buildBtn");
  if (buildBtn) buildBtn.textContent = "Create profile";
  byId("nameFieldWrap")?.classList.remove("hidden");
  byId("loanFields")?.classList.remove("hidden");
  byId("resetWrap")?.classList.add("hidden");
  syncDeleteProfileButton(false);
  syncCopyProfileButton(false);
  hideShareEditor();
  syncOpenProfileButton(null);
  highlightGutterSelection(null);
  const profileName = byId<HTMLInputElement>("profileName");
  const principal = byId<HTMLInputElement>("principal");
  const rate = byId<HTMLInputElement>("rate");
  const term = byId<HTMLInputElement>("term");
  if (profileName) profileName.value = nextProfileName();
  if (principal) principal.value = "400000";
  if (rate) rate.value = "6.5";
  if (term) term.value = "30";
  const start = byId<HTMLInputElement>("startDate");
  if (start) start.value = start.dataset.default || start.value;
  const autoMark = byId<HTMLInputElement>("autoMarkDuePaid");
  if (autoMark) autoMark.checked = false;
  const error = byId("error");
  if (error) error.textContent = "";
}

export function selectProfile(profileId: string): void {
  const opt = profileOptionById(profileId);
  if (!opt) {
    selectCreateMode();
    return;
  }
  const owner = isOptionOwner(opt);
  setLoanFormMode("edit", profileId);
  const buildBtn = byId("buildBtn");
  if (buildBtn) buildBtn.textContent = "Save changes";
  byId("nameFieldWrap")?.classList.remove("hidden");
  byId("loanFields")?.classList.remove("hidden");
  byId("resetWrap")?.classList.remove("hidden");
  syncDeleteProfileButton(owner);
  syncCopyProfileButton(true);
  const profileName = byId<HTMLInputElement>("profileName");
  const principal = byId<HTMLInputElement>("principal");
  const rate = byId<HTMLInputElement>("rate");
  const term = byId<HTMLInputElement>("term");
  const startDate = byId<HTMLInputElement>("startDate");
  if (profileName) profileName.value = opt.dataset.name || "";
  if (principal) principal.value = opt.dataset.principal || "400000";
  if (rate) rate.value = opt.dataset.rate || "6.5";
  if (term) term.value = opt.dataset.term || "30";
  if (startDate) startDate.value = opt.dataset.start || "";
  const autoMark = byId<HTMLInputElement>("autoMarkDuePaid");
  if (autoMark) autoMark.checked = opt.dataset.autoMarkDue === "true";
  const error = byId("error");
  if (error) error.textContent = "";
  syncOpenProfileButton(profileId);
  highlightGutterSelection(profileId);
  loadSharePanel(profileId);
}

export function openSelectedProfile(): void {
  const form = byId<HTMLFormElement>("loanForm");
  const profileId = form?.dataset.profileId;
  const select = byId<HTMLSelectElement>("profileSelect");
  if (!profileId || !select || form.dataset.mode !== "edit") return;
  if (select.value === profileId) {
    popover()?.hidePopover();
    return;
  }
  select.value = profileId;
  popover()?.hidePopover();
  if (typeof htmx !== "undefined") {
    htmx.trigger(select, "change");
  } else {
    select.dispatchEvent(new Event("change", { bubbles: true }));
  }
}

/** Fill the profile manager UI without showing the popover. */
export function prepareProfileManager({
  mode = "edit",
  profileId,
}: OpenProfileManagerOptions = {}): void {
  renderProfileGutter();
  const title = byId("popoverTitle");
  if (title) title.textContent = "Profiles";

  const options = profileSelectOptions();
  const activeId =
    profileId ||
    byId<HTMLSelectElement>("profileSelect")?.value ||
    dashboard()?.dataset.profileId ||
    "";

  if (mode === "create" || !options.length) {
    selectCreateMode();
  } else {
    selectProfile(activeId || options[0]!.value);
  }
}

/** Show the loan popover after the current event turn (avoids click light-dismiss). */
export function showLoanPopover(): void {
  const pop = popover();
  if (!pop) return;
  const reveal = () => {
    if (!pop.matches(":popover-open")) {
      pop.showPopover();
    } else if (Number(getComputedStyle(pop).opacity) < 1) {
      // Stuck mid-animation / invisible: force a clean reopen.
      pop.hidePopover();
      pop.showPopover();
    }
    byId<HTMLInputElement>("profileName")?.focus();
  };
  requestAnimationFrame(reveal);
}

export function openProfileManager(
  options: OpenProfileManagerOptions = {},
): void {
  prepareProfileManager(options);
  showLoanPopover();
}

export function openCreate(): void {
  openProfileManager({ mode: "create" });
}

export function openEdit(): void {
  const id =
    dashboard()?.dataset.profileId ||
    byId<HTMLSelectElement>("profileSelect")?.value ||
    "";
  if (!id) {
    openCreate();
    return;
  }
  openProfileManager({ mode: "edit", profileId: id });
}

// Re-export types used by app.ts bindings for convenience.
export type { CopyProfileData, DeleteProfileData, LoanFormMode };
