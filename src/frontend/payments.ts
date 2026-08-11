import {
  asElement,
  byId,
  dashboard,
  loadingButtonsFor,
  panelEl,
  setButtonsLoading,
  storeHtmxLoadingButtons,
  takeHtmxLoadingButtons,
} from "./core";

type YearOpenOverrides = Record<string, boolean>;

interface PaidSnapshot {
  key: string;
  paid: boolean;
}

interface PaymentsScrollSnapshot {
  type: "table" | "window";
  top: number;
  left: number;
}

const paidSnapshots = new WeakMap<Element, PaidSnapshot>();

let scrolledToCurrentPayment = false;
let applyingYearCollapse = false;
let paymentsScrollRestore: PaymentsScrollSnapshot | null = null;

export function isPaidToggle(
  elt: Element | null | undefined,
): elt is HTMLElement {
  return elt instanceof HTMLElement && elt.classList.contains("paid-toggle");
}

export function syncExtraFormOptions(form: HTMLFormElement): void {
  const recast = form.querySelector('input[name="recast"]');
  const recurring = form.querySelector('input[name="recurring"]');
  const recurrence = form.querySelector('select[name="recurrence"]');
  if (
    !(recast instanceof HTMLInputElement) ||
    !(recurring instanceof HTMLInputElement) ||
    !(recurrence instanceof HTMLSelectElement)
  ) {
    return;
  }

  if (recurring.checked) {
    recast.checked = false;
    recast.disabled = true;
  } else {
    recast.disabled = false;
  }

  if (recast.checked) {
    recurring.checked = false;
    recurring.disabled = true;
  } else {
    recurring.disabled = false;
  }

  recurrence.disabled = !recurring.checked;
  recast.closest("label")?.classList.toggle("is-disabled", recast.disabled);
  recurring
    .closest("label")
    ?.classList.toggle("is-disabled", recurring.disabled);
}

function isServerPaidToggle(elt: HTMLElement): boolean {
  if (elt.closest("#panel-payments")) return true;
  if (elt.classList.contains("extra")) return true;
  const key = paidToggleKey(elt);
  return key.startsWith("extra:");
}

function paymentFilter(): string {
  return dashboard()?.dataset.filter || "all";
}

function paidToggleKey(elt: HTMLElement | null | undefined): string {
  return elt?.dataset.payKey || elt?.dataset.pay || "";
}

function paidTogglesForKey(key: string): HTMLElement[] {
  if (!key) return [];
  return [...document.querySelectorAll<HTMLElement>(".paid-toggle")].filter(
    (btn) => paidToggleKey(btn) === key,
  );
}

function setPaidToggleUi(elt: HTMLElement, paid: boolean): void {
  if (elt.classList.contains("pay-chip")) {
    const unpaidStatus = elt.dataset.unpaidStatus || "future";
    const unpaidText = elt.dataset.unpaidStatusText || "Upcoming";
    elt.classList.remove("paid", "due", "missed", "future");
    elt.classList.add(paid ? "paid" : unpaidStatus);
    elt.setAttribute("aria-pressed", paid ? "true" : "false");
    const statusText = paid
      ? elt.classList.contains("extra")
        ? unpaidText.includes("Recast")
          ? "Recast · Paid"
          : "Extra · Paid"
        : "Paid"
      : unpaidText;
    const statusEl = elt.querySelector(".chip-status");
    if (statusEl) statusEl.textContent = statusText;
    const amount = elt.querySelector("strong")?.textContent || "";
    const aria = paid ? "Mark unpaid" : "Mark paid";
    elt.setAttribute("aria-label", `${amount}, ${statusText}. ${aria}`);
    return;
  }

  elt.classList.toggle("paid", paid);
  const label = paid ? "Mark unpaid" : "Mark paid";
  elt.setAttribute("aria-label", label);
  elt.title = label;

  const row = elt.closest("tr");
  const card = elt.closest(".pay-card");
  row?.classList.toggle("paid-row", paid);
  card?.classList.toggle("paid", paid);

  const filter = paymentFilter();
  const hide =
    (filter === "unpaid" && paid) || (filter === "paid" && !paid);
  row?.classList.toggle("filter-hidden", hide);
  card?.classList.toggle("filter-hidden", hide);
}

function applyPaidToggleKey(key: string, paid: boolean): void {
  paidTogglesForKey(key).forEach((btn) => setPaidToggleUi(btn, paid));
}

function paymentsSurface(
  panel: HTMLElement | null = panelEl("payments"),
): HTMLElement | null {
  return panel?.querySelector<HTMLElement>(".payments-panel") ?? null;
}

function yearOpenStorageKey(profileId: string): string {
  return `payments-year-open:${profileId || "default"}`;
}

function yearModeStorageKey(profileId: string): string {
  return `payments-year-mode:${profileId || "default"}`;
}

function readYearOpenOverrides(profileId: string): YearOpenOverrides {
  try {
    const raw = sessionStorage.getItem(yearOpenStorageKey(profileId));
    if (!raw) return {};
    const parsed: unknown = JSON.parse(raw);
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      return {};
    }
    const out: YearOpenOverrides = {};
    for (const [key, value] of Object.entries(
      parsed as Record<string, unknown>,
    )) {
      out[key] = Boolean(value);
    }
    return out;
  } catch {
    return {};
  }
}

function writeYearOpenOverrides(
  profileId: string,
  overrides: YearOpenOverrides,
): void {
  try {
    sessionStorage.setItem(
      yearOpenStorageKey(profileId),
      JSON.stringify(overrides),
    );
  } catch {
    /* ignore quota / private mode */
  }
}

function clearYearOpenOverrides(profileId: string): void {
  try {
    sessionStorage.removeItem(yearOpenStorageKey(profileId));
  } catch {
    /* ignore */
  }
}

export function setYearGroupExpanded(
  group: HTMLElement | null | undefined,
  expanded: boolean,
): void {
  if (!group) return;
  const year = group.dataset.year;
  group.dataset.expanded = expanded ? "true" : "false";
  if (group.tagName === "DETAILS") {
    (group as HTMLDetailsElement).open = expanded;
  } else {
    group.classList.toggle("is-collapsed", !expanded);
    const toggle = group.querySelector(".year-toggle");
    if (toggle) {
      toggle.setAttribute("aria-expanded", expanded ? "true" : "false");
    }
  }
  if (year) {
    const panel = panelEl("payments");
    panel
      ?.querySelectorAll<HTMLElement>(`.pay-year-group[data-year="${year}"]`)
      .forEach((other) => {
        if (other === group) return;
        other.dataset.expanded = expanded ? "true" : "false";
        if (other.tagName === "DETAILS") {
          (other as HTMLDetailsElement).open = expanded;
        } else {
          other.classList.toggle("is-collapsed", !expanded);
          const toggle = other.querySelector(".year-toggle");
          if (toggle) {
            toggle.setAttribute("aria-expanded", expanded ? "true" : "false");
          }
        }
      });
  }
}

export function applyPaymentYearCollapse(
  panel: HTMLElement | null = panelEl("payments"),
): void {
  const surface = paymentsSurface(panel);
  if (!surface) return;
  const profileId = surface.dataset.profileId || "";
  const mode = surface.dataset.paymentsYearExpand || "current";
  try {
    const modeKey = yearModeStorageKey(profileId);
    const prevMode = sessionStorage.getItem(modeKey);
    if (prevMode && prevMode !== mode) {
      clearYearOpenOverrides(profileId);
    }
    sessionStorage.setItem(modeKey, mode);
  } catch {
    /* ignore */
  }
  const overrides = readYearOpenOverrides(profileId);
  const years = new Set<string>();
  surface
    .querySelectorAll<HTMLElement>(".pay-year-group[data-year]")
    .forEach((group) => {
      if (group.dataset.year) years.add(group.dataset.year);
    });
  applyingYearCollapse = true;
  try {
    years.forEach((year) => {
      const group = primaryYearGroup(surface, year);
      if (!group) return;
      const serverExpanded = group.dataset.expanded === "true";
      const expanded = Object.prototype.hasOwnProperty.call(overrides, year)
        ? !!overrides[year]
        : serverExpanded;
      setYearGroupExpanded(group, expanded);
    });
  } finally {
    applyingYearCollapse = false;
  }
}

export function rememberYearExpanded(
  year: string | undefined,
  expanded: boolean,
): void {
  if (applyingYearCollapse) return;
  const surface = paymentsSurface();
  if (!surface || year == null) return;
  const profileId = surface.dataset.profileId || "";
  const overrides = readYearOpenOverrides(profileId);
  overrides[String(year)] = expanded;
  writeYearOpenOverrides(profileId, overrides);
}

export function isYearGroupExpanded(group: HTMLElement): boolean {
  if (group.tagName === "DETAILS") return (group as HTMLDetailsElement).open;
  return !group.classList.contains("is-collapsed");
}

function primaryYearGroup(
  surface: HTMLElement,
  year: string,
): HTMLElement | null {
  const desktop = surface.querySelector<HTMLElement>(".payments-desktop");
  const tbody = surface.querySelector<HTMLElement>(
    `tbody.pay-year-group[data-year="${year}"]`,
  );
  if (tbody && desktop && getComputedStyle(desktop).display !== "none") {
    return tbody;
  }
  const details = surface.querySelector<HTMLElement>(
    `details.pay-year-group[data-year="${year}"]`,
  );
  return details || tbody;
}

export function isPaymentsPanelSwap(
  elt: Element | null | undefined,
  requestConfig: HtmxRequestConfig | undefined,
): boolean {
  const target = requestConfig?.target;
  if (target instanceof HTMLElement && target.id === "panel-payments") {
    return true;
  }
  if (
    typeof target === "string" &&
    target.replace(/^#/, "") === "panel-payments"
  ) {
    return true;
  }
  let node: Element | null = elt ?? null;
  while (node) {
    if (node instanceof HTMLElement) {
      const hxTarget = node.getAttribute("hx-target");
      if (hxTarget === "#panel-payments") return true;
    }
    node = node.parentElement;
  }
  return false;
}

function snapshotPaymentYearExpansion(
  panel: HTMLElement | null = panelEl("payments"),
): void {
  const surface = paymentsSurface(panel);
  if (!surface) return;
  const profileId = surface.dataset.profileId || "";
  const overrides = readYearOpenOverrides(profileId);
  const years = new Set<string>();
  surface
    .querySelectorAll<HTMLElement>(".pay-year-group[data-year]")
    .forEach((group) => {
      if (group.dataset.year) years.add(group.dataset.year);
    });
  years.forEach((year) => {
    const group = primaryYearGroup(surface, year);
    if (group) overrides[year] = isYearGroupExpanded(group);
  });
  writeYearOpenOverrides(profileId, overrides);
}

function paymentsUsesTableScroll(surface: HTMLElement | null): boolean {
  const desktop = surface?.querySelector<HTMLElement>(".payments-desktop");
  const tableWrap = surface?.querySelector("#paymentsTableWrap");
  return !!(
    desktop &&
    tableWrap &&
    getComputedStyle(desktop).display !== "none"
  );
}

function snapshotPaymentsScroll(
  panel: HTMLElement | null = panelEl("payments"),
): PaymentsScrollSnapshot | null {
  const surface = paymentsSurface(panel);
  if (!surface) return null;
  if (paymentsUsesTableScroll(surface)) {
    const tableWrap = surface.querySelector("#paymentsTableWrap");
    if (!(tableWrap instanceof HTMLElement)) return null;
    return {
      type: "table",
      top: tableWrap.scrollTop,
      left: tableWrap.scrollLeft,
    };
  }
  return { type: "window", top: window.scrollY, left: window.scrollX };
}

export function snapshotPaymentsPanelState(
  panel: HTMLElement | null = panelEl("payments"),
): void {
  snapshotPaymentYearExpansion(panel);
  paymentsScrollRestore = snapshotPaymentsScroll(panel);
}

export function clearPaymentsScrollRestore(): void {
  paymentsScrollRestore = null;
}

export function restorePaymentsScroll(
  panel: HTMLElement | null = panelEl("payments"),
): void {
  const saved = paymentsScrollRestore;
  paymentsScrollRestore = null;
  if (!saved) return;
  const surface = paymentsSurface(panel);
  if (!surface) return;
  if (saved.type === "table" && paymentsUsesTableScroll(surface)) {
    const tableWrap = surface.querySelector("#paymentsTableWrap");
    if (!(tableWrap instanceof HTMLElement)) return;
    tableWrap.scrollTop = saved.top;
    tableWrap.scrollLeft = saved.left;
    return;
  }
  if (saved.type === "window") {
    window.scrollTo({
      top: saved.top,
      left: saved.left,
      behavior: "instant" as ScrollBehavior,
    });
  }
}

function expandYearForCurrentMonth(
  panel: HTMLElement | null = panelEl("payments"),
): void {
  const target = panel?.querySelector(".payment-current-month");
  if (!target) return;
  const group = target.closest<HTMLElement>(".pay-year-group");
  if (!group) return;
  setYearGroupExpanded(group, true);
  rememberYearExpanded(group.dataset.year, true);
}

export function maybeScrollToCurrentMonthPayment(): void {
  if (scrolledToCurrentPayment) return;
  const panel = panelEl("payments");
  if (!panel?.classList.contains("active")) return;
  applyPaymentYearCollapse(panel);
  expandYearForCurrentMonth(panel);
  const target = [...panel.querySelectorAll(".payment-current-month")].find(
    (el) => el.getClientRects().length > 0,
  );
  scrolledToCurrentPayment = true;
  if (!target) return;
  requestAnimationFrame(() => {
    target.scrollIntoView({ block: "center", behavior: "smooth" });
  });
}

function paymentsActionsPopover(): HTMLElement | null {
  return byId("paymentsActionsPopover");
}

function positionPaymentsActionsPopover(): void {
  const pop = paymentsActionsPopover();
  const btn = byId("paymentsActionsBtn");
  if (!(pop instanceof HTMLElement) || !(btn instanceof HTMLElement)) return;
  const rect = btn.getBoundingClientRect();
  const gap = 6;
  const width = pop.offsetWidth || 216;
  const left = Math.min(
    Math.max(8, rect.right - width),
    window.innerWidth - width - 8,
  );
  pop.style.top = `${Math.round(rect.bottom + gap)}px`;
  pop.style.left = `${Math.round(left)}px`;
}

export function closePaymentsActionsPopover(): void {
  paymentsActionsPopover()?.hidePopover?.();
}

export function bindPaymentsActionsPopover(root: ParentNode = document): void {
  const fromRoot =
    root instanceof Element && root.id === "paymentsActionsPopover"
      ? root
      : root.querySelector?.("#paymentsActionsPopover");
  const pop = fromRoot || document.getElementById("paymentsActionsPopover");
  if (!(pop instanceof HTMLElement) || pop.dataset.bound === "true") return;
  pop.dataset.bound = "true";
  pop.addEventListener("toggle", (e) => {
    const toggle = e as ToggleEvent;
    if (toggle.newState === "open") {
      positionPaymentsActionsPopover();
      byId("paymentsActionsBtn")?.setAttribute("aria-expanded", "true");
    } else {
      byId("paymentsActionsBtn")?.setAttribute("aria-expanded", "false");
    }
  });
  pop.addEventListener("click", (e) => {
    const action = asElement(e.target)?.closest<HTMLButtonElement>(
      ".payments-action",
    );
    if (!action || action.disabled) return;
    closePaymentsActionsPopover();
  });
}

function notePopover(): HTMLElement | null {
  return byId("notePopover");
}

export function openNotePopover(btn: HTMLElement): void {
  const dash = dashboard();
  const profileId = dash?.dataset.profileId;
  if (!profileId) return;

  let note = "";
  try {
    const parsed: unknown = JSON.parse(
      btn.getAttribute("data-note-json") || '""',
    );
    note = typeof parsed === "string" ? parsed : "";
  } catch {
    note = "";
  }

  const form = byId<HTMLFormElement>("noteForm");
  if (!form) return;
  form.action = `/profiles/${profileId}/notes`;
  form.setAttribute("hx-post", `/profiles/${profileId}/notes`);
  form.setAttribute("hx-target", "#panel-payments");
  form.setAttribute("hx-swap", "innerHTML show:none");

  const notePayKey = byId<HTMLInputElement>("notePayKey");
  const noteFilter = byId<HTMLInputElement>("noteFilter");
  const noteYear = byId<HTMLInputElement>("noteYear");
  const noteGrain = byId<HTMLInputElement>("noteGrain");
  if (notePayKey) notePayKey.value = btn.dataset.payKey || "";
  if (noteFilter) noteFilter.value = dash.dataset.filter || "all";
  if (noteYear) {
    noteYear.value = dash.dataset.year || String(new Date().getFullYear());
  }
  if (noteGrain) noteGrain.value = dash.dataset.grain || "monthly";
  const noteText = byId<HTMLTextAreaElement>("noteText");
  if (noteText) {
    noteText.value = note;
    noteText.setCustomValidity(
      note.length > 500 ? "Notes are limited to 500 characters." : "",
    );
  }
  const dueEl = byId("notePopoverDue");
  if (dueEl) {
    dueEl.textContent = btn.dataset.due ? `Due ${btn.dataset.due}` : "";
  }
  const titleEl = byId("notePopoverTitle");
  if (titleEl) {
    titleEl.textContent = note.trim() ? "Edit note" : "Add note";
  }

  if (typeof htmx !== "undefined") htmx.process(form);
  notePopover()?.showPopover();
  noteText?.focus();
}

export function closeNotePopover(): void {
  notePopover()?.hidePopover();
}

/** Paid-toggle optimistic UI + payments panel scroll snapshotting. */
export function installPaymentsListeners(): void {
  document.addEventListener("htmx:beforeRequest", (e) => {
    if (isPaymentsPanelSwap(e.detail.elt, e.detail.requestConfig)) {
      snapshotPaymentsPanelState();
    }
    const elt = e.detail.elt;
    if (!isPaidToggle(elt)) return;

    // Payments panel / extras: wait for the panel swap instead of flipping locally.
    if (isServerPaidToggle(elt)) {
      const buttons = loadingButtonsFor(
        elt,
        e.detail.requestConfig?.triggeringEvent,
      );
      if (buttons.length) {
        storeHtmxLoadingButtons(elt, buttons);
        setButtonsLoading(buttons, true);
      }
      return;
    }
    const paid = elt.classList.contains("paid");
    const snapshot: PaidSnapshot = { key: paidToggleKey(elt), paid };
    paidSnapshots.set(elt, snapshot);
    applyPaidToggleKey(snapshot.key, !paid);
  });

  document.addEventListener("htmx:afterRequest", (e) => {
    if (
      !e.detail.successful &&
      isPaymentsPanelSwap(e.detail.elt, e.detail.requestConfig)
    ) {
      clearPaymentsScrollRestore();
    }
    const elt = e.detail.elt;
    if (!isPaidToggle(elt)) return;

    const snapshot = paidSnapshots.get(elt);
    if (snapshot) {
      if (!e.detail.successful) {
        applyPaidToggleKey(snapshot.key, snapshot.paid);
      }
      paidSnapshots.delete(elt);
    }
    // Extra toggles may have used the loading-button path; clear it if still present
    // (successful panel swaps replace the button, so this mainly covers failures).
    const buttons = takeHtmxLoadingButtons(elt);
    if (buttons) setButtonsLoading(buttons, false);
  });
}
