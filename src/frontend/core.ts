import type {
  ActivateTabOptions,
  MarkPanelsStaleDetail,
  TabActivationHooks,
  TabId,
} from "./types";

export type LoadingButton = HTMLButtonElement | HTMLInputElement;

export const TAB_IDS = [
  "summary",
  "calendar",
  "payments",
  "improvements",
  "chart",
] as const satisfies readonly TabId[];

const htmxLoadingButtons = new WeakMap<Element, LoadingButton[]>();

let tabHooks: TabActivationHooks = {};

export function setTabActivationHooks(hooks: TabActivationHooks): void {
  tabHooks = hooks;
}

export function byId<T extends HTMLElement = HTMLElement>(
  id: string,
): T | null {
  return document.getElementById(id) as T | null;
}

export function asElement(target: EventTarget | null): Element | null {
  return target instanceof Element ? target : null;
}

export function csrfToken(): string {
  const meta = document.querySelector('meta[name="csrf-token"]');
  return meta instanceof HTMLMetaElement ? meta.content || "" : "";
}

export function money(n: number): string {
  return Number(n).toLocaleString(undefined, {
    style: "currency",
    currency: "USD",
    maximumFractionDigits: 2,
  });
}

export function normalizeTab(tab: string | null | undefined): TabId {
  return TAB_IDS.includes(tab as TabId) ? (tab as TabId) : "calendar";
}

export function dashboard(): HTMLElement | null {
  return byId("dashboard");
}

export function panelEl(id: TabId): HTMLElement | null {
  return byId(`panel-${id}`);
}

export function syncDashboardMetaFromDom(): void {
  const dash = dashboard();
  if (!dash) return;
  const yearLabel = document.querySelector("#panel-calendar .month-label");
  if (yearLabel?.textContent?.trim()) {
    dash.dataset.year = yearLabel.textContent.trim();
  }
  const filter = document.querySelector<HTMLSelectElement>(
    "#panel-payments select[name='filter']",
  );
  if (filter?.value) {
    dash.dataset.filter = filter.value;
  }
  const grainBtn = document.querySelector<HTMLElement>(
    "#panel-chart [data-grain].active",
  );
  if (grainBtn?.dataset.grain) {
    dash.dataset.grain = grainBtn.dataset.grain;
  }
  const scopeBtn = document.querySelector<HTMLElement>(
    "#panel-summary [data-summary-scope].active",
  );
  if (scopeBtn?.dataset.summaryScope) {
    dash.dataset.scope = scopeBtn.dataset.summaryScope;
  }
}

export function syncProfileBarFromDashboard(): void {
  const meta = byId("profileMeta");
  if (!meta) return;
  const dash = dashboard();
  const principal = meta.querySelector('[data-field="principal"]');
  const rate = meta.querySelector('[data-field="rate"]');
  const term = meta.querySelector('[data-field="term"]');
  if (principal) principal.textContent = dash?.dataset.principalDisplay || "";
  if (rate) rate.textContent = dash?.dataset.rateDisplay || "";
  if (term) term.textContent = dash?.dataset.termDisplay || "";
  meta.hidden = !dash;
}

export function partialUrl(id: TabId): string {
  const dash = dashboard();
  const year = dash?.dataset.year || String(new Date().getFullYear());
  const filter = dash?.dataset.filter || "all";
  const grain = dash?.dataset.grain || "monthly";
  const scope = dash?.dataset.scope || "year";
  const urls: Record<TabId, string> = {
    summary: `/partials/summary?tab=summary&year=${encodeURIComponent(year)}&filter=${encodeURIComponent(filter)}&grain=${encodeURIComponent(grain)}&scope=${encodeURIComponent(scope)}`,
    calendar: `/partials/calendar?year=${encodeURIComponent(year)}&tab=calendar&filter=${encodeURIComponent(filter)}&grain=${encodeURIComponent(grain)}`,
    payments: `/partials/payments?tab=payments&filter=${encodeURIComponent(filter)}&year=${encodeURIComponent(year)}&grain=${encodeURIComponent(grain)}`,
    improvements: `/partials/improvements?tab=improvements&year=${encodeURIComponent(year)}&filter=${encodeURIComponent(filter)}&grain=${encodeURIComponent(grain)}`,
    chart: `/partials/chart?tab=chart&grain=${encodeURIComponent(grain)}&year=${encodeURIComponent(year)}&filter=${encodeURIComponent(filter)}`,
  };
  return urls[id];
}

export function markPanelsStale({
  keep,
  invalidateChart,
}: MarkPanelsStaleDetail): void {
  for (const id of TAB_IDS) {
    const panel = panelEl(id);
    if (!panel) continue;
    if (id === keep) {
      panel.dataset.stale = "false";
      continue;
    }
    if (id === "chart" && !invalidateChart) {
      continue;
    }
    panel.dataset.stale = "true";
  }
}

export async function refreshPanelIfStale(id: TabId): Promise<void> {
  const panel = panelEl(id);
  if (!panel || panel.dataset.stale !== "true") return;
  syncDashboardMetaFromDom();
  const url = partialUrl(id);
  if (!url || typeof htmx === "undefined") return;
  await htmx.ajax("GET", url, { target: panel, swap: "innerHTML" });
  panel.dataset.stale = "false";
}

export function activateTab(
  tabId: string | null | undefined,
  { focus = true, syncUrl = true }: ActivateTabOptions = {},
): void {
  const id = normalizeTab(tabId);
  const target = document.querySelector<HTMLElement>(`.tab[data-tab="${id}"]`);
  if (!target) return;
  document.querySelectorAll<HTMLElement>(".tab").forEach((t) => {
    const selected = t === target;
    t.classList.toggle("active", selected);
    t.setAttribute("aria-selected", selected ? "true" : "false");
    t.tabIndex = selected ? 0 : -1;
  });
  document
    .querySelectorAll(".panel")
    .forEach((p) => p.classList.remove("active"));
  const panel = panelEl(id);
  if (panel) panel.classList.add("active");
  const dash = dashboard();
  if (dash) dash.dataset.tab = id;
  if (focus) target.focus();
  if (syncUrl) {
    const url = new URL(location.href);
    url.searchParams.set("tab", id);
    url.hash = "";
    const next = `${url.pathname}${url.search}`;
    if (`${location.pathname}${location.search}${location.hash}` !== next) {
      history.replaceState(null, "", next);
    }
  }
  const refresh = refreshPanelIfStale(id);
  if (id === "payments") {
    Promise.resolve(refresh).finally(() => tabHooks.onPaymentsActivated?.());
  }
  if (id === "chart") {
    Promise.resolve(refresh).finally(() => tabHooks.onChartActivated?.());
  }
}

export function closeMenu(menuId: string, btnId: string): void {
  const menu = byId(menuId);
  const btn = byId(btnId);
  if (menu) menu.classList.remove("open");
  if (btn) btn.setAttribute("aria-expanded", "false");
}

export function closeAccountMenu(): void {
  closeMenu("accountMenu", "accountMenuBtn");
}

export function toggleMenu(menuId: string, btn: HTMLElement): void {
  const menu = byId(menuId);
  if (!menu) return;
  const open = !menu.classList.contains("open");
  menu.classList.toggle("open", open);
  btn.setAttribute("aria-expanded", open ? "true" : "false");
}

export function loadingButtonsFor(
  elt: Element | null | undefined,
  evt: Event | undefined,
): LoadingButton[] {
  const submitter =
    evt && "submitter" in evt ? (evt as SubmitEvent).submitter : null;
  if (
    submitter instanceof HTMLButtonElement ||
    submitter instanceof HTMLInputElement
  ) {
    return [submitter];
  }
  if (elt instanceof HTMLButtonElement) return [elt];
  if (elt instanceof HTMLInputElement && elt.type === "submit") return [elt];
  const form =
    elt instanceof HTMLFormElement
      ? elt
      : elt instanceof Element
        ? elt.closest("form")
        : null;
  if (!form) return [];
  return Array.from(
    form.querySelectorAll<LoadingButton>(
      'button[type="submit"], input[type="submit"]',
    ),
  );
}

export function setButtonsLoading(
  buttons: LoadingButton[],
  loading: boolean,
): void {
  for (const btn of buttons) {
    btn.disabled = loading;
    btn.classList.toggle("is-loading", loading);
    if (loading) btn.setAttribute("aria-busy", "true");
    else btn.removeAttribute("aria-busy");
  }
}

export function beginHtmxLoading(
  elt: Element,
  triggeringEvent: Event | undefined,
): void {
  const buttons = loadingButtonsFor(elt, triggeringEvent);
  if (!buttons.length) return;
  htmxLoadingButtons.set(elt, buttons);
  setButtonsLoading(buttons, true);
}

export function endHtmxLoading(
  elt: Element,
  triggeringEvent: Event | undefined,
): void {
  const buttons =
    htmxLoadingButtons.get(elt) || loadingButtonsFor(elt, triggeringEvent);
  setButtonsLoading(buttons, false);
  htmxLoadingButtons.delete(elt);
}

export function takeHtmxLoadingButtons(
  elt: Element,
): LoadingButton[] | undefined {
  const buttons = htmxLoadingButtons.get(elt);
  if (buttons) htmxLoadingButtons.delete(elt);
  return buttons;
}

export function storeHtmxLoadingButtons(
  elt: Element,
  buttons: LoadingButton[],
): void {
  htmxLoadingButtons.set(elt, buttons);
}

function isPaidToggleElement(elt: Element | null | undefined): boolean {
  return elt instanceof HTMLElement && elt.classList.contains("paid-toggle");
}

/** CSRF injection + generic HTMX submit-button loading (skips paid toggles). */
export function installCoreListeners(): void {
  document.addEventListener("htmx:configRequest", (e) => {
    const token = csrfToken();
    if (token) {
      e.detail.headers["X-CSRF-Token"] = token;
    }
  });

  document.addEventListener(
    "submit",
    (e) => {
      const form = e.target;
      if (!(form instanceof HTMLFormElement)) return;
      if ((form.method || "get").toLowerCase() === "get") return;
      const token = csrfToken();
      if (!token) return;
      let input = form.querySelector<HTMLInputElement>(
        'input[name="csrf_token"]',
      );
      if (!input) {
        input = document.createElement("input");
        input.type = "hidden";
        input.name = "csrf_token";
        form.appendChild(input);
      }
      input.value = token;
    },
    true,
  );

  document.addEventListener("htmx:beforeRequest", (e) => {
    const elt = e.detail.elt;
    if (isPaidToggleElement(elt)) return;
    beginHtmxLoading(elt, e.detail.requestConfig?.triggeringEvent);
  });

  document.addEventListener("htmx:afterRequest", (e) => {
    const elt = e.detail.elt;
    if (isPaidToggleElement(elt)) return;
    endHtmxLoading(elt, e.detail.requestConfig?.triggeringEvent);
  });
}
