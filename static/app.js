"use strict";
(() => {
  // src/frontend/core.ts
  var TAB_IDS = [
    "summary",
    "calendar",
    "payments",
    "improvements",
    "chart"
  ];
  var htmxLoadingButtons = /* @__PURE__ */ new WeakMap();
  var tabHooks = {};
  function setTabActivationHooks(hooks) {
    tabHooks = hooks;
  }
  function byId(id) {
    return document.getElementById(id);
  }
  function asElement(target) {
    return target instanceof Element ? target : null;
  }
  function csrfToken() {
    const meta = document.querySelector('meta[name="csrf-token"]');
    return meta instanceof HTMLMetaElement ? meta.content || "" : "";
  }
  function money(n) {
    return Number(n).toLocaleString(void 0, {
      style: "currency",
      currency: "USD",
      maximumFractionDigits: 2
    });
  }
  function normalizeTab(tab) {
    return TAB_IDS.includes(tab) ? tab : "calendar";
  }
  function dashboard() {
    return byId("dashboard");
  }
  function panelEl(id) {
    return byId(`panel-${id}`);
  }
  function syncDashboardMetaFromDom() {
    const dash = dashboard();
    if (!dash) return;
    const yearLabel = document.querySelector("#panel-calendar .month-label");
    if (yearLabel?.textContent?.trim()) {
      dash.dataset.year = yearLabel.textContent.trim();
    }
    const filter = document.querySelector(
      "#panel-payments select[name='filter']"
    );
    if (filter?.value) {
      dash.dataset.filter = filter.value;
    }
    const grainBtn = document.querySelector(
      "#panel-chart [data-grain].active"
    );
    if (grainBtn?.dataset.grain) {
      dash.dataset.grain = grainBtn.dataset.grain;
    }
    const scopeBtn = document.querySelector(
      "#panel-summary [data-summary-scope].active"
    );
    if (scopeBtn?.dataset.summaryScope) {
      dash.dataset.scope = scopeBtn.dataset.summaryScope;
    }
  }
  function syncProfileBarFromDashboard() {
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
  function partialUrl(id) {
    const dash = dashboard();
    const year = dash?.dataset.year || String((/* @__PURE__ */ new Date()).getFullYear());
    const filter = dash?.dataset.filter || "all";
    const grain = dash?.dataset.grain || "monthly";
    const scope = dash?.dataset.scope || "year";
    const urls = {
      summary: `/partials/summary?tab=summary&year=${encodeURIComponent(year)}&filter=${encodeURIComponent(filter)}&grain=${encodeURIComponent(grain)}&scope=${encodeURIComponent(scope)}`,
      calendar: `/partials/calendar?year=${encodeURIComponent(year)}&tab=calendar&filter=${encodeURIComponent(filter)}&grain=${encodeURIComponent(grain)}`,
      payments: `/partials/payments?tab=payments&filter=${encodeURIComponent(filter)}&year=${encodeURIComponent(year)}&grain=${encodeURIComponent(grain)}`,
      improvements: `/partials/improvements?tab=improvements&year=${encodeURIComponent(year)}&filter=${encodeURIComponent(filter)}&grain=${encodeURIComponent(grain)}`,
      chart: `/partials/chart?tab=chart&grain=${encodeURIComponent(grain)}&year=${encodeURIComponent(year)}&filter=${encodeURIComponent(filter)}`
    };
    return urls[id];
  }
  function markPanelsStale({
    keep,
    invalidateChart
  }) {
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
  async function refreshPanelIfStale(id) {
    const panel = panelEl(id);
    if (!panel || panel.dataset.stale !== "true") return;
    syncDashboardMetaFromDom();
    const url = partialUrl(id);
    if (!url || typeof htmx === "undefined") return;
    await htmx.ajax("GET", url, { target: panel, swap: "innerHTML" });
    panel.dataset.stale = "false";
  }
  function activateTab(tabId, { focus = true, syncUrl = true } = {}) {
    const id = normalizeTab(tabId);
    const target = document.querySelector(`.tab[data-tab="${id}"]`);
    if (!target) return;
    document.querySelectorAll(".tab").forEach((t) => {
      const selected = t === target;
      t.classList.toggle("active", selected);
      t.setAttribute("aria-selected", selected ? "true" : "false");
      t.tabIndex = selected ? 0 : -1;
    });
    document.querySelectorAll(".panel").forEach((p) => p.classList.remove("active"));
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
  function closeMenu(menuId, btnId) {
    const menu = byId(menuId);
    const btn = byId(btnId);
    if (menu) menu.classList.remove("open");
    if (btn) btn.setAttribute("aria-expanded", "false");
  }
  function closeAccountMenu() {
    closeMenu("accountMenu", "accountMenuBtn");
  }
  function toggleMenu(menuId, btn) {
    const menu = byId(menuId);
    if (!menu) return;
    const open = !menu.classList.contains("open");
    menu.classList.toggle("open", open);
    btn.setAttribute("aria-expanded", open ? "true" : "false");
  }
  function loadingButtonsFor(elt, evt) {
    const submitter = evt && "submitter" in evt ? evt.submitter : null;
    if (submitter instanceof HTMLButtonElement || submitter instanceof HTMLInputElement) {
      return [submitter];
    }
    if (elt instanceof HTMLButtonElement) return [elt];
    if (elt instanceof HTMLInputElement && elt.type === "submit") return [elt];
    const form = elt instanceof HTMLFormElement ? elt : elt instanceof Element ? elt.closest("form") : null;
    if (!form) return [];
    return Array.from(
      form.querySelectorAll(
        'button[type="submit"], input[type="submit"]'
      )
    );
  }
  function setButtonsLoading(buttons, loading) {
    for (const btn of buttons) {
      btn.disabled = loading;
      btn.classList.toggle("is-loading", loading);
      if (loading) btn.setAttribute("aria-busy", "true");
      else btn.removeAttribute("aria-busy");
    }
  }
  function beginHtmxLoading(elt, triggeringEvent) {
    const buttons = loadingButtonsFor(elt, triggeringEvent);
    if (!buttons.length) return;
    htmxLoadingButtons.set(elt, buttons);
    setButtonsLoading(buttons, true);
  }
  function endHtmxLoading(elt, triggeringEvent) {
    const buttons = htmxLoadingButtons.get(elt) || loadingButtonsFor(elt, triggeringEvent);
    setButtonsLoading(buttons, false);
    htmxLoadingButtons.delete(elt);
  }
  function takeHtmxLoadingButtons(elt) {
    const buttons = htmxLoadingButtons.get(elt);
    if (buttons) htmxLoadingButtons.delete(elt);
    return buttons;
  }
  function storeHtmxLoadingButtons(elt, buttons) {
    htmxLoadingButtons.set(elt, buttons);
  }
  function isPaidToggleElement(elt) {
    return elt instanceof HTMLElement && elt.classList.contains("paid-toggle");
  }
  function installCoreListeners() {
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
        let input = form.querySelector(
          'input[name="csrf_token"]'
        );
        if (!input) {
          input = document.createElement("input");
          input.type = "hidden";
          input.name = "csrf_token";
          form.appendChild(input);
        }
        input.value = token;
      },
      true
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

  // src/frontend/chart.ts
  var breakdownChart = null;
  function chartGrain() {
    return dashboard()?.dataset.grain === "yearly" ? "yearly" : "monthly";
  }
  function cssVar(name) {
    return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  }
  function parseBuckets(raw) {
    try {
      const data = JSON.parse(raw || "[]");
      if (!Array.isArray(data)) return [];
      return data.filter((row) => {
        if (!row || typeof row !== "object") return false;
        const r = row;
        return typeof r.label === "string" && typeof r.year === "number" && typeof r.principal === "number" && typeof r.interest === "number" && typeof r.payment === "number";
      });
    } catch {
      return [];
    }
  }
  function destroyBreakdownChart() {
    if (!breakdownChart) return;
    try {
      breakdownChart.destroy();
    } catch {
    }
    breakdownChart = null;
  }
  function breakdownTooltipHtml(params) {
    const datum = params?.datum ?? params?.[0]?.datum;
    if (!datum) return "";
    const countRow = datum.count != null ? `<div class="row"><span>Payments</span><span>${datum.count}</span></div>` : "";
    return `
      <div class="chart-tooltip-card">
        <strong>${datum.label}</strong>
        <div class="row"><span>Principal</span><span>${money(datum.principal)}</span></div>
        <div class="row"><span>Interest</span><span>${money(datum.interest)}</span></div>
        <div class="row"><span>Payment</span><span>${money(datum.payment)}</span></div>
        ${countRow}
      </div>`;
  }
  function yearAxisTickValues(data) {
    const firstLabelByYear = /* @__PURE__ */ new Map();
    for (const row of data) {
      if (row?.year == null || firstLabelByYear.has(row.year)) continue;
      firstLabelByYear.set(row.year, row.label);
    }
    const years = [...firstLabelByYear.keys()].sort((a, b) => a - b);
    const ticks = [];
    for (let i = 0; i < years.length; i += 2) {
      const year = years[i];
      if (year == null) continue;
      const label = firstLabelByYear.get(year);
      if (label != null) ticks.push(label);
    }
    return ticks;
  }
  function breakdownChartOptions(container, data) {
    const sea = cssVar("--sea") || "#2f6f6a";
    const sand = cssVar("--sand") || "#d4c4a8";
    const inkSoft = cssVar("--ink-soft") || "#3d4f55";
    const fontBody = cssVar("--font-body") || "Outfit, sans-serif";
    const yearByLabel = new Map(data.map((row) => [row.label, row.year]));
    const yearTicks = yearAxisTickValues(data);
    return {
      container,
      data,
      padding: { top: 8, right: 12, bottom: 8, left: 8 },
      series: [
        {
          type: "bar",
          xKey: "label",
          yKey: "principal",
          yName: "Principal",
          stacked: true,
          fill: sea,
          strokeWidth: 0,
          cornerRadius: 0,
          tooltip: { renderer: breakdownTooltipHtml }
        },
        {
          type: "bar",
          xKey: "label",
          yKey: "interest",
          yName: "Interest",
          stacked: true,
          fill: sand,
          strokeWidth: 0,
          cornerRadius: 0,
          tooltip: { renderer: breakdownTooltipHtml }
        }
      ],
      axes: {
        x: {
          type: "category",
          paddingInner: data.length > 90 ? 0.05 : 0.2,
          paddingOuter: 0.05,
          interval: yearTicks.length ? { values: yearTicks } : void 0,
          label: {
            color: inkSoft,
            fontFamily: fontBody,
            fontSize: 11,
            avoidCollisions: false,
            formatter: ({ value }) => {
              const year = yearByLabel.get(value);
              if (year != null) return String(year);
              const match = String(value).match(/(\d{4})\s*$/);
              return match?.[1] ? match[1] : String(value);
            }
          },
          line: { enabled: false },
          tick: { enabled: false }
        },
        y: {
          type: "number",
          label: {
            color: inkSoft,
            fontFamily: fontBody,
            fontSize: 11,
            formatter: ({ value }) => money(value)
          },
          gridLine: {
            style: [{ stroke: cssVar("--line") || "rgba(28, 42, 46, 0.12)" }]
          },
          line: { enabled: false },
          tick: { enabled: false }
        }
      },
      legend: {
        position: "top",
        spacing: 12,
        item: {
          marker: { shape: "square", size: 10 },
          label: {
            color: inkSoft,
            fontFamily: fontBody,
            fontSize: 12
          }
        }
      },
      tooltip: {
        mode: "single"
      }
    };
  }
  function renderBreakdownChart() {
    const panel = panelEl("chart");
    const wrap = byId("chartWrap");
    const container = byId("breakdownChart");
    if (!panel?.classList.contains("active") || !wrap || !container) return;
    if (typeof agCharts === "undefined" || !agCharts.AgCharts) return;
    const grain = chartGrain();
    const data = parseBuckets(
      grain === "yearly" ? wrap.dataset.yearlyBuckets : wrap.dataset.monthlyBuckets
    );
    const options = breakdownChartOptions(container, data);
    if (breakdownChart) {
      breakdownChart.update(options);
      return;
    }
    breakdownChart = agCharts.AgCharts.create(options);
  }
  function setChartGrain(grain) {
    const panel = panelEl("chart");
    if (!panel) return;
    const next = grain === "yearly" ? "yearly" : "monthly";
    panel.querySelectorAll(".seg-toggle [data-grain]").forEach((btn) => {
      const on = btn.dataset.grain === next;
      btn.classList.toggle("active", on);
      btn.setAttribute("aria-pressed", on ? "true" : "false");
    });
    panel.querySelectorAll("[data-grain-hint]").forEach((el) => {
      el.classList.toggle("hidden", el.dataset.grainHint !== next);
    });
    const dash = dashboard();
    if (dash) dash.dataset.grain = next;
    renderBreakdownChart();
  }

  // src/frontend/payments.ts
  var paidSnapshots = /* @__PURE__ */ new WeakMap();
  var scrolledToCurrentPayment = false;
  var applyingYearCollapse = false;
  var paymentsScrollRestore = null;
  function isPaidToggle(elt) {
    return elt instanceof HTMLElement && elt.classList.contains("paid-toggle");
  }
  function syncExtraFormOptions(form) {
    const recast = form.querySelector('input[name="recast"]');
    const recurring = form.querySelector('input[name="recurring"]');
    const recurrence = form.querySelector('select[name="recurrence"]');
    if (!(recast instanceof HTMLInputElement) || !(recurring instanceof HTMLInputElement) || !(recurrence instanceof HTMLSelectElement)) {
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
    recurring.closest("label")?.classList.toggle("is-disabled", recurring.disabled);
  }
  function isServerPaidToggle(elt) {
    if (elt.closest("#panel-payments")) return true;
    if (elt.classList.contains("extra")) return true;
    const key = paidToggleKey(elt);
    return key.startsWith("extra:");
  }
  function paymentFilter() {
    return dashboard()?.dataset.filter || "all";
  }
  function paidToggleKey(elt) {
    return elt?.dataset.payKey || elt?.dataset.pay || "";
  }
  function paidTogglesForKey(key) {
    if (!key) return [];
    return [...document.querySelectorAll(".paid-toggle")].filter(
      (btn) => paidToggleKey(btn) === key
    );
  }
  function setPaidToggleUi(elt, paid) {
    if (elt.classList.contains("pay-chip")) {
      const unpaidStatus = elt.dataset.unpaidStatus || "future";
      const unpaidText = elt.dataset.unpaidStatusText || "Upcoming";
      elt.classList.remove("paid", "due", "missed", "future");
      elt.classList.add(paid ? "paid" : unpaidStatus);
      elt.setAttribute("aria-pressed", paid ? "true" : "false");
      const statusText = paid ? elt.classList.contains("extra") ? unpaidText.includes("Recast") ? "Recast \xB7 Paid" : "Extra \xB7 Paid" : "Paid" : unpaidText;
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
    const hide = filter === "unpaid" && paid || filter === "paid" && !paid;
    row?.classList.toggle("filter-hidden", hide);
    card?.classList.toggle("filter-hidden", hide);
  }
  function applyPaidToggleKey(key, paid) {
    paidTogglesForKey(key).forEach((btn) => setPaidToggleUi(btn, paid));
  }
  function paymentsSurface(panel = panelEl("payments")) {
    return panel?.querySelector(".payments-panel") ?? null;
  }
  function yearOpenStorageKey(profileId) {
    return `payments-year-open:${profileId || "default"}`;
  }
  function yearModeStorageKey(profileId) {
    return `payments-year-mode:${profileId || "default"}`;
  }
  function readYearOpenOverrides(profileId) {
    try {
      const raw = sessionStorage.getItem(yearOpenStorageKey(profileId));
      if (!raw) return {};
      const parsed = JSON.parse(raw);
      if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
        return {};
      }
      const out = {};
      for (const [key, value] of Object.entries(
        parsed
      )) {
        out[key] = Boolean(value);
      }
      return out;
    } catch {
      return {};
    }
  }
  function writeYearOpenOverrides(profileId, overrides) {
    try {
      sessionStorage.setItem(
        yearOpenStorageKey(profileId),
        JSON.stringify(overrides)
      );
    } catch {
    }
  }
  function clearYearOpenOverrides(profileId) {
    try {
      sessionStorage.removeItem(yearOpenStorageKey(profileId));
    } catch {
    }
  }
  function setYearGroupExpanded(group, expanded) {
    if (!group) return;
    const year = group.dataset.year;
    group.dataset.expanded = expanded ? "true" : "false";
    if (group.tagName === "DETAILS") {
      group.open = expanded;
    } else {
      group.classList.toggle("is-collapsed", !expanded);
      const toggle = group.querySelector(".year-toggle");
      if (toggle) {
        toggle.setAttribute("aria-expanded", expanded ? "true" : "false");
      }
    }
    if (year) {
      const panel = panelEl("payments");
      panel?.querySelectorAll(`.pay-year-group[data-year="${year}"]`).forEach((other) => {
        if (other === group) return;
        other.dataset.expanded = expanded ? "true" : "false";
        if (other.tagName === "DETAILS") {
          other.open = expanded;
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
  function applyPaymentYearCollapse(panel = panelEl("payments")) {
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
    }
    const overrides = readYearOpenOverrides(profileId);
    const years = /* @__PURE__ */ new Set();
    surface.querySelectorAll(".pay-year-group[data-year]").forEach((group) => {
      if (group.dataset.year) years.add(group.dataset.year);
    });
    applyingYearCollapse = true;
    try {
      years.forEach((year) => {
        const group = primaryYearGroup(surface, year);
        if (!group) return;
        const serverExpanded = group.dataset.expanded === "true";
        const expanded = Object.prototype.hasOwnProperty.call(overrides, year) ? !!overrides[year] : serverExpanded;
        setYearGroupExpanded(group, expanded);
      });
    } finally {
      applyingYearCollapse = false;
    }
  }
  function rememberYearExpanded(year, expanded) {
    if (applyingYearCollapse) return;
    const surface = paymentsSurface();
    if (!surface || year == null) return;
    const profileId = surface.dataset.profileId || "";
    const overrides = readYearOpenOverrides(profileId);
    overrides[String(year)] = expanded;
    writeYearOpenOverrides(profileId, overrides);
  }
  function isYearGroupExpanded(group) {
    if (group.tagName === "DETAILS") return group.open;
    return !group.classList.contains("is-collapsed");
  }
  function primaryYearGroup(surface, year) {
    const desktop = surface.querySelector(".payments-desktop");
    const tbody = surface.querySelector(
      `tbody.pay-year-group[data-year="${year}"]`
    );
    if (tbody && desktop && getComputedStyle(desktop).display !== "none") {
      return tbody;
    }
    const details = surface.querySelector(
      `details.pay-year-group[data-year="${year}"]`
    );
    return details || tbody;
  }
  function isPaymentsPanelSwap(elt, requestConfig) {
    const target = requestConfig?.target;
    if (target instanceof HTMLElement && target.id === "panel-payments") {
      return true;
    }
    if (typeof target === "string" && target.replace(/^#/, "") === "panel-payments") {
      return true;
    }
    let node = elt ?? null;
    while (node) {
      if (node instanceof HTMLElement) {
        const hxTarget = node.getAttribute("hx-target");
        if (hxTarget === "#panel-payments") return true;
      }
      node = node.parentElement;
    }
    return false;
  }
  function snapshotPaymentYearExpansion(panel = panelEl("payments")) {
    const surface = paymentsSurface(panel);
    if (!surface) return;
    const profileId = surface.dataset.profileId || "";
    const overrides = readYearOpenOverrides(profileId);
    const years = /* @__PURE__ */ new Set();
    surface.querySelectorAll(".pay-year-group[data-year]").forEach((group) => {
      if (group.dataset.year) years.add(group.dataset.year);
    });
    years.forEach((year) => {
      const group = primaryYearGroup(surface, year);
      if (group) overrides[year] = isYearGroupExpanded(group);
    });
    writeYearOpenOverrides(profileId, overrides);
  }
  function paymentsUsesTableScroll(surface) {
    const desktop = surface?.querySelector(".payments-desktop");
    const tableWrap = surface?.querySelector("#paymentsTableWrap");
    return !!(desktop && tableWrap && getComputedStyle(desktop).display !== "none");
  }
  function snapshotPaymentsScroll(panel = panelEl("payments")) {
    const surface = paymentsSurface(panel);
    if (!surface) return null;
    if (paymentsUsesTableScroll(surface)) {
      const tableWrap = surface.querySelector("#paymentsTableWrap");
      if (!(tableWrap instanceof HTMLElement)) return null;
      return {
        type: "table",
        top: tableWrap.scrollTop,
        left: tableWrap.scrollLeft
      };
    }
    return { type: "window", top: window.scrollY, left: window.scrollX };
  }
  function snapshotPaymentsPanelState(panel = panelEl("payments")) {
    snapshotPaymentYearExpansion(panel);
    paymentsScrollRestore = snapshotPaymentsScroll(panel);
  }
  function clearPaymentsScrollRestore() {
    paymentsScrollRestore = null;
  }
  function restorePaymentsScroll(panel = panelEl("payments")) {
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
        behavior: "instant"
      });
    }
  }
  function expandYearForCurrentMonth(panel = panelEl("payments")) {
    const target = panel?.querySelector(".payment-current-month");
    if (!target) return;
    const group = target.closest(".pay-year-group");
    if (!group) return;
    setYearGroupExpanded(group, true);
    rememberYearExpanded(group.dataset.year, true);
  }
  function maybeScrollToCurrentMonthPayment() {
    if (scrolledToCurrentPayment) return;
    const panel = panelEl("payments");
    if (!panel?.classList.contains("active")) return;
    applyPaymentYearCollapse(panel);
    expandYearForCurrentMonth(panel);
    const target = [...panel.querySelectorAll(".payment-current-month")].find(
      (el) => el.getClientRects().length > 0
    );
    scrolledToCurrentPayment = true;
    if (!target) return;
    requestAnimationFrame(() => {
      target.scrollIntoView({ block: "center", behavior: "smooth" });
    });
  }
  function paymentsActionsPopover() {
    return byId("paymentsActionsPopover");
  }
  function positionPaymentsActionsPopover() {
    const pop = paymentsActionsPopover();
    const btn = byId("paymentsActionsBtn");
    if (!(pop instanceof HTMLElement) || !(btn instanceof HTMLElement)) return;
    const rect = btn.getBoundingClientRect();
    const gap = 6;
    const width = pop.offsetWidth || 216;
    const left = Math.min(
      Math.max(8, rect.right - width),
      window.innerWidth - width - 8
    );
    pop.style.top = `${Math.round(rect.bottom + gap)}px`;
    pop.style.left = `${Math.round(left)}px`;
  }
  function closePaymentsActionsPopover() {
    paymentsActionsPopover()?.hidePopover?.();
  }
  function bindPaymentsActionsPopover(root = document) {
    const fromRoot = root instanceof Element && root.id === "paymentsActionsPopover" ? root : root.querySelector?.("#paymentsActionsPopover");
    const pop = fromRoot || document.getElementById("paymentsActionsPopover");
    if (!(pop instanceof HTMLElement) || pop.dataset.bound === "true") return;
    pop.dataset.bound = "true";
    pop.addEventListener("toggle", (e) => {
      const toggle = e;
      if (toggle.newState === "open") {
        positionPaymentsActionsPopover();
        byId("paymentsActionsBtn")?.setAttribute("aria-expanded", "true");
      } else {
        byId("paymentsActionsBtn")?.setAttribute("aria-expanded", "false");
      }
    });
    pop.addEventListener("click", (e) => {
      const action = asElement(e.target)?.closest(
        ".payments-action"
      );
      if (!action || action.disabled) return;
      closePaymentsActionsPopover();
    });
  }
  function notePopover() {
    return byId("notePopover");
  }
  function openNotePopover(btn) {
    const dash = dashboard();
    const profileId = dash?.dataset.profileId;
    if (!profileId) return;
    let note = "";
    try {
      const parsed = JSON.parse(
        btn.getAttribute("data-note-json") || '""'
      );
      note = typeof parsed === "string" ? parsed : "";
    } catch {
      note = "";
    }
    const form = byId("noteForm");
    if (!form) return;
    form.action = `/profiles/${profileId}/notes`;
    form.setAttribute("hx-post", `/profiles/${profileId}/notes`);
    form.setAttribute("hx-target", "#panel-payments");
    form.setAttribute("hx-swap", "innerHTML show:none");
    const notePayKey = byId("notePayKey");
    const noteFilter = byId("noteFilter");
    const noteYear = byId("noteYear");
    const noteGrain = byId("noteGrain");
    if (notePayKey) notePayKey.value = btn.dataset.payKey || "";
    if (noteFilter) noteFilter.value = dash.dataset.filter || "all";
    if (noteYear) {
      noteYear.value = dash.dataset.year || String((/* @__PURE__ */ new Date()).getFullYear());
    }
    if (noteGrain) noteGrain.value = dash.dataset.grain || "monthly";
    const noteText = byId("noteText");
    if (noteText) {
      noteText.value = note;
      noteText.setCustomValidity(
        note.length > 500 ? "Notes are limited to 500 characters." : ""
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
  function closeNotePopover() {
    notePopover()?.hidePopover();
  }
  function installPaymentsListeners() {
    document.addEventListener("htmx:beforeRequest", (e) => {
      if (isPaymentsPanelSwap(e.detail.elt, e.detail.requestConfig)) {
        snapshotPaymentsPanelState();
      }
      const elt = e.detail.elt;
      if (!isPaidToggle(elt)) return;
      if (isServerPaidToggle(elt)) {
        const buttons = loadingButtonsFor(
          elt,
          e.detail.requestConfig?.triggeringEvent
        );
        if (buttons.length) {
          storeHtmxLoadingButtons(elt, buttons);
          setButtonsLoading(buttons, true);
        }
        return;
      }
      const paid = elt.classList.contains("paid");
      const snapshot = { key: paidToggleKey(elt), paid };
      paidSnapshots.set(elt, snapshot);
      applyPaidToggleKey(snapshot.key, !paid);
    });
    document.addEventListener("htmx:afterRequest", (e) => {
      if (!e.detail.successful && isPaymentsPanelSwap(e.detail.elt, e.detail.requestConfig)) {
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
      const buttons = takeHtmxLoadingButtons(elt);
      if (buttons) setButtonsLoading(buttons, false);
    });
  }

  // src/frontend/profiles.ts
  function popover() {
    return byId("loanPopover");
  }
  function improvementPopover() {
    return byId("improvementPopover");
  }
  function parseJsonAttr(el, name) {
    try {
      const parsed = JSON.parse(el.getAttribute(name) || '""');
      return typeof parsed === "string" ? parsed : "";
    } catch {
      return "";
    }
  }
  function openImprovementPopover(btn, mode = "edit") {
    const dash = dashboard();
    const profileId = dash?.dataset.profileId;
    if (!profileId) return;
    const form = byId("improvementForm");
    if (!form) return;
    const title = byId("improvementPopoverTitle");
    const isAdd = mode === "add";
    let action;
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
    const dateEl = byId("improvementDate");
    const amountEl = byId("improvementAmount");
    if (dateEl) dateEl.value = btn.dataset.date || "";
    if (amountEl) amountEl.value = isAdd ? "" : btn.dataset.amount || "";
    const noteEl = byId("improvementNote");
    const detailEl = byId("improvementDetail");
    const note = isAdd ? "" : parseJsonAttr(btn, "data-note-json");
    const detail = isAdd ? "" : parseJsonAttr(btn, "data-detail-json");
    if (noteEl) {
      noteEl.value = note;
      noteEl.setCustomValidity(
        note.length > 200 ? "Improvement notes are limited to 200 characters." : ""
      );
    }
    if (detailEl) {
      detailEl.value = detail;
      detailEl.setCustomValidity(
        detail.length > 1e3 ? "Improvement details are limited to 1000 characters." : ""
      );
    }
    if (typeof htmx !== "undefined") htmx.process(form);
    improvementPopover()?.showPopover();
    byId(isAdd ? "improvementDate" : "improvementNote")?.focus();
  }
  function closeImprovementPopover() {
    improvementPopover()?.hidePopover();
  }
  function loanFormAction(mode, profileId) {
    if (mode === "edit" && profileId) return `/profiles/${profileId}`;
    return "/profiles";
  }
  function setLoanFormMode(mode, profileId) {
    const form = byId("loanForm");
    const btn = byId("buildBtn");
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
  function nextProfileName() {
    const select = byId("profileSelect");
    const count = [...select?.options || []].filter((o) => o.value).length;
    return `Profile ${count + 1}`;
  }
  function profileSelectOptions() {
    const select = byId("profileSelect");
    return [...select?.options || []].filter((o) => o.value);
  }
  function profileOptionById(profileId) {
    return profileSelectOptions().find((o) => o.value === profileId) || null;
  }
  function isOptionOwner(opt) {
    if (opt?.dataset.shared != null) return opt.dataset.shared !== "true";
    const bar = byId("profileBar");
    return bar?.dataset.isOwner !== "false";
  }
  function isActiveOwner() {
    const select = byId("profileSelect");
    return isOptionOwner(select?.selectedOptions?.[0]);
  }
  function syncOwnerState() {
    const owner = isActiveOwner();
    const bar = byId("profileBar");
    if (bar) bar.dataset.isOwner = owner ? "true" : "false";
  }
  function hideShareEditor() {
    const wrap = byId("shareEditor");
    const panel = byId("sharePanel");
    wrap?.classList.add("hidden");
    if (panel) panel.innerHTML = "";
  }
  function fillShareInviteUrl() {
    const input = byId("shareInviteUrl");
    if (!input) return;
    const path = input.dataset.path || "";
    if (path) input.value = `${window.location.origin}${path}`;
  }
  function loadSharePanel(profileId) {
    const wrap = byId("shareEditor");
    const panel = byId("sharePanel");
    if (!wrap || !panel || !profileId) return;
    wrap.classList.remove("hidden");
    if (typeof htmx === "undefined") return;
    htmx.ajax("GET", `/profiles/${profileId}/share-panel`, {
      target: "#sharePanel",
      swap: "innerHTML"
    });
  }
  function syncOpenProfileButton(profileId) {
    const openBtn = byId("openProfileBtn");
    if (!openBtn) return;
    const select = byId("profileSelect");
    const activeId = select?.value || dashboard()?.dataset.profileId || "";
    const canOpen = Boolean(profileId) && profileId !== activeId;
    openBtn.classList.toggle("hidden", !canOpen);
    openBtn.disabled = !canOpen;
  }
  function highlightGutterSelection(profileId) {
    const list = byId("profileGutterList");
    if (!list) return;
    list.querySelectorAll(".profile-gutter-item").forEach((btn) => {
      const on = profileId != null && btn.dataset.profileId === profileId;
      btn.classList.toggle("active", on);
      btn.setAttribute("aria-current", on ? "true" : "false");
    });
    const newBtn = byId("profileGutterNewBtn");
    newBtn?.classList.toggle("active", profileId == null);
  }
  function renderProfileGutter() {
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
        const name = opt.dataset.name || opt.textContent?.replace(/\s*\(shared\)\s*$/, "") || "Profile";
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
      })
    );
  }
  function setCanCreateProfile(allowed) {
    const actions = document.querySelector(".profile-gutter-actions");
    if (actions) actions.dataset.canCreate = allowed ? "true" : "false";
    const newBtn = byId("profileGutterNewBtn");
    if (newBtn) {
      newBtn.disabled = !allowed;
      newBtn.title = allowed ? "New profile" : "Pro feature";
    }
    const copyBtn = byId("copyProfileBtn");
    if (copyBtn && !copyBtn.classList.contains("hidden")) {
      copyBtn.disabled = !allowed;
      copyBtn.title = allowed ? "Copy profile" : "Pro feature";
    }
  }
  function canCreateProfile() {
    const actions = document.querySelector(".profile-gutter-actions");
    if (actions?.dataset.canCreate != null) {
      return actions.dataset.canCreate === "true";
    }
    return !byId("profileGutterNewBtn")?.disabled;
  }
  function syncProfileSelect(profiles, selectedId) {
    const select = byId("profileSelect");
    const bar = byId("profileBar");
    if (!select) return;
    const previous = select.value;
    select.replaceChildren(
      ...profiles.length ? profiles.map((p) => {
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
      }) : [
        (() => {
          const opt = document.createElement("option");
          opt.value = "";
          opt.textContent = "No profiles yet";
          return opt;
        })()
      ]
    );
    const nextId = selectedId && profiles.some((p) => p.id === selectedId) ? selectedId : previous && profiles.some((p) => p.id === previous) ? previous : profiles[0]?.id || "";
    select.value = nextId;
    select.disabled = profiles.length === 0;
    bar?.classList.toggle("hidden", profiles.length === 0);
    syncOwnerState();
  }
  function upsertProfileOption(profile) {
    const select = byId("profileSelect");
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
    opt.textContent = profile.is_shared ? `${profile.name} (shared)` : profile.name;
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
  function refreshDashboardForActive(activeId) {
    const select = byId("profileSelect");
    if (!select) return;
    if (activeId) {
      if (select.value !== activeId) select.value = activeId;
      if (typeof htmx !== "undefined") htmx.trigger(select, "change");
      else select.dispatchEvent(new Event("change", { bubbles: true }));
      return;
    }
    window.location.href = "/";
  }
  async function postProfileJson(url) {
    const headers = {
      Accept: "application/json",
      "HX-Request": "true"
    };
    const token = csrfToken();
    if (token) headers["X-CSRF-Token"] = token;
    const res = await fetch(url, {
      method: "POST",
      headers,
      credentials: "same-origin"
    });
    let body = null;
    try {
      body = await res.json();
    } catch {
      body = null;
    }
    if (!res.ok || !body || !("ok" in body) || !body.ok) {
      const message = body && "error" in body && typeof body.error === "string" ? body.error : "Request failed.";
      throw new Error(message);
    }
    return body.data;
  }
  function showProfileManagerError(message) {
    const err = byId("error");
    if (err) err.textContent = message || "";
  }
  function syncDeleteProfileButton(visible) {
    byId("deleteProfileBtn")?.classList.toggle("hidden", !visible);
  }
  function syncCopyProfileButton(visible) {
    const btn = byId("copyProfileBtn");
    if (!btn) return;
    btn.classList.toggle("hidden", !visible);
    const allowed = canCreateProfile();
    btn.disabled = !allowed;
    btn.title = allowed ? "Copy profile" : "Pro feature";
  }
  function selectCreateMode() {
    setLoanFormMode("create");
    const buildBtn = byId("buildBtn");
    if (buildBtn) buildBtn.textContent = "Create profile";
    byId("nameFieldWrap")?.classList.remove("hidden");
    byId("loanFields")?.classList.remove("hidden");
    syncDeleteProfileButton(false);
    syncCopyProfileButton(false);
    hideShareEditor();
    syncOpenProfileButton(null);
    highlightGutterSelection(null);
    const profileName = byId("profileName");
    const principal = byId("principal");
    const rate = byId("rate");
    const term = byId("term");
    if (profileName) profileName.value = nextProfileName();
    if (principal) principal.value = "400000";
    if (rate) rate.value = "6.5";
    if (term) term.value = "30";
    const start = byId("startDate");
    if (start) start.value = start.dataset.default || start.value;
    const autoMark = byId("autoMarkDuePaid");
    if (autoMark) autoMark.checked = false;
    const error = byId("error");
    if (error) error.textContent = "";
  }
  function selectProfile(profileId) {
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
    syncDeleteProfileButton(owner);
    syncCopyProfileButton(true);
    const profileName = byId("profileName");
    const principal = byId("principal");
    const rate = byId("rate");
    const term = byId("term");
    const startDate = byId("startDate");
    if (profileName) profileName.value = opt.dataset.name || "";
    if (principal) principal.value = opt.dataset.principal || "400000";
    if (rate) rate.value = opt.dataset.rate || "6.5";
    if (term) term.value = opt.dataset.term || "30";
    if (startDate) startDate.value = opt.dataset.start || "";
    const autoMark = byId("autoMarkDuePaid");
    if (autoMark) autoMark.checked = opt.dataset.autoMarkDue === "true";
    const error = byId("error");
    if (error) error.textContent = "";
    syncOpenProfileButton(profileId);
    highlightGutterSelection(profileId);
    loadSharePanel(profileId);
  }
  function openSelectedProfile() {
    const form = byId("loanForm");
    const profileId = form?.dataset.profileId;
    const select = byId("profileSelect");
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
  function prepareProfileManager({
    mode = "edit",
    profileId
  } = {}) {
    renderProfileGutter();
    const title = byId("popoverTitle");
    if (title) title.textContent = "Profiles";
    const options = profileSelectOptions();
    const activeId = profileId || byId("profileSelect")?.value || dashboard()?.dataset.profileId || "";
    if (mode === "create" || !options.length) {
      selectCreateMode();
    } else {
      selectProfile(activeId || options[0].value);
    }
  }
  function showLoanPopover() {
    const pop = popover();
    if (!pop) return;
    const reveal = () => {
      if (!pop.matches(":popover-open")) {
        pop.showPopover();
      } else if (Number(getComputedStyle(pop).opacity) < 1) {
        pop.hidePopover();
        pop.showPopover();
      }
      byId("profileName")?.focus();
    };
    requestAnimationFrame(reveal);
  }
  function openProfileManager(options = {}) {
    prepareProfileManager(options);
    showLoanPopover();
  }
  function openCreate() {
    openProfileManager({ mode: "create" });
  }
  function openEdit() {
    const id = dashboard()?.dataset.profileId || byId("profileSelect")?.value || "";
    if (!id) {
      openCreate();
      return;
    }
    openProfileManager({ mode: "edit", profileId: id });
  }

  // src/frontend/app.ts
  setTabActivationHooks({
    onPaymentsActivated: maybeScrollToCurrentMonthPayment,
    onChartActivated: renderBreakdownChart
  });
  installCoreListeners();
  installPaymentsListeners();
  function bindUi() {
    document.body.addEventListener("htmx:configRequest", (e) => {
      const elt = e.detail.elt;
      if (!isPaidToggle(elt) || e.detail.verb !== "post") return;
      e.detail.parameters = e.detail.parameters || {};
      e.detail.parameters.paid = !elt.classList.contains("paid");
    });
    byId("loanForm")?.addEventListener("submit", (e) => {
      const form = e.currentTarget;
      const mode = form.dataset.mode || "create";
      const profileId = form.dataset.profileId;
      if (mode === "edit" && !profileId) {
        e.preventDefault();
        const error = byId("error");
        if (error) error.textContent = "No profile selected.";
        return;
      }
      setLoanFormMode(mode, profileId);
    });
    byId("manageProfilesBtn")?.addEventListener("click", () => {
      prepareProfileManager({ mode: "edit" });
      showLoanPopover();
    });
    byId("profileGutterNewBtn")?.addEventListener("click", () => {
      selectCreateMode();
      byId("profileName")?.focus();
    });
    byId("profileGutterList")?.addEventListener("click", (e) => {
      const btn = asElement(e.target)?.closest(
        ".profile-gutter-item"
      );
      if (!btn) return;
      const id = btn.dataset.profileId;
      if (id) selectProfile(id);
    });
    byId("openProfileBtn")?.addEventListener("click", openSelectedProfile);
    byId("profileSelect")?.addEventListener("change", syncOwnerState);
    syncOwnerState();
    byId("emptyNewBtn")?.addEventListener("click", (e) => {
      const btn = e.currentTarget;
      if (btn.dataset.emptyAction === "edit") openEdit();
      else openCreate();
    });
    byId("closePopoverBtn")?.addEventListener("click", () => {
      popover()?.hidePopover();
    });
    byId("closeNotePopoverBtn")?.addEventListener("click", closeNotePopover);
    byId("clearNoteBtn")?.addEventListener("click", () => {
      const text = byId("noteText");
      if (text) {
        text.value = "";
        text.setCustomValidity("");
      }
      text?.focus();
    });
    byId("noteText")?.addEventListener("input", (e) => {
      const text = e.currentTarget;
      if (text.value.length > 500) {
        text.setCustomValidity("Notes are limited to 500 characters.");
      } else {
        text.setCustomValidity("");
      }
    });
    byId("noteForm")?.addEventListener("submit", (e) => {
      const text = byId("noteText");
      if (!text) return;
      if (text.value.length > 500) {
        text.setCustomValidity("Notes are limited to 500 characters.");
        text.reportValidity();
        e.preventDefault();
      } else {
        text.setCustomValidity("");
      }
    });
    byId("noteForm")?.addEventListener("htmx:afterRequest", (e) => {
      if (e.detail.successful) closeNotePopover();
    });
    byId("closeImprovementPopoverBtn")?.addEventListener(
      "click",
      closeImprovementPopover
    );
    byId("cancelImprovementBtn")?.addEventListener(
      "click",
      closeImprovementPopover
    );
    byId("improvementNote")?.addEventListener(
      "input",
      (e) => {
        const el = e.currentTarget;
        el.setCustomValidity(
          el.value.length > 200 ? "Improvement notes are limited to 200 characters." : ""
        );
      }
    );
    byId("improvementDetail")?.addEventListener(
      "input",
      (e) => {
        const el = e.currentTarget;
        el.setCustomValidity(
          el.value.length > 1e3 ? "Improvement details are limited to 1000 characters." : ""
        );
      }
    );
    byId("improvementForm")?.addEventListener("submit", (e) => {
      const noteEl = byId("improvementNote");
      const detailEl = byId("improvementDetail");
      let invalid = null;
      if (noteEl) {
        if (noteEl.value.length > 200) {
          noteEl.setCustomValidity(
            "Improvement notes are limited to 200 characters."
          );
          invalid = noteEl;
        } else {
          noteEl.setCustomValidity("");
        }
      }
      if (detailEl) {
        if (detailEl.value.length > 1e3) {
          detailEl.setCustomValidity(
            "Improvement details are limited to 1000 characters."
          );
          invalid = invalid || detailEl;
        } else {
          detailEl.setCustomValidity("");
        }
      }
      if (invalid) {
        invalid.reportValidity();
        e.preventDefault();
      }
    });
    byId("improvementForm")?.addEventListener("htmx:afterRequest", (e) => {
      if (e.detail.successful) closeImprovementPopover();
    });
    byId("copyProfileBtn")?.addEventListener("click", async () => {
      const form = byId("loanForm");
      const id = form?.dataset.profileId;
      if (!id || form.dataset.mode !== "edit" || !canCreateProfile()) return;
      showProfileManagerError("");
      try {
        const data = await postProfileJson(
          `/profiles/${id}/copy`
        );
        upsertProfileOption(data.profile);
        setCanCreateProfile(Boolean(data.can_create_profile));
        renderProfileGutter();
        selectProfile(data.profile.id);
      } catch (err) {
        const message = err instanceof Error ? err.message : "Could not copy profile.";
        showProfileManagerError(message);
      }
    });
    byId("deleteProfileBtn")?.addEventListener("click", async () => {
      const form = byId("loanForm");
      const id = form?.dataset.profileId;
      if (!id || form.dataset.mode !== "edit") return;
      const opt = profileOptionById(id);
      if (!isOptionOwner(opt)) return;
      const name = opt?.dataset.name || "this profile";
      if (!confirm(`Delete profile \u201C${name}\u201D? This cannot be undone.`)) return;
      const select = byId("profileSelect");
      const wasActive = select?.value === id || dashboard()?.dataset.profileId === id;
      showProfileManagerError("");
      try {
        const data = await postProfileJson(
          `/profiles/${id}/delete`
        );
        const profiles = Array.isArray(data.profiles) ? data.profiles : [];
        setCanCreateProfile(Boolean(data.can_create_profile));
        syncProfileSelect(profiles, data.active_id || profiles[0]?.id || "");
        renderProfileGutter();
        if (profiles.length && profiles[0]) {
          selectProfile(profiles[0].id);
        } else {
          selectCreateMode();
        }
        if (wasActive) {
          refreshDashboardForActive(data.active_id || profiles[0]?.id || "");
        }
      } catch (err) {
        const message = err instanceof Error ? err.message : "Could not delete profile.";
        showProfileManagerError(message);
      }
    });
    byId("sharePanel")?.addEventListener("click", (e) => {
      const btn = asElement(e.target)?.closest("#copyShareLinkBtn");
      if (!btn) return;
      const input = byId("shareInviteUrl");
      fillShareInviteUrl();
      const value = input?.value || "";
      if (!value) return;
      void navigator.clipboard?.writeText(value).then(() => {
        btn.textContent = "Copied";
        setTimeout(() => {
          btn.textContent = "Copy link";
        }, 1500);
      });
    });
    document.body.addEventListener("htmx:afterSwap", (e) => {
      if (e.detail?.target?.id === "sharePanel") {
        fillShareInviteUrl();
      }
    });
    byId("accountMenuBtn")?.addEventListener("click", (e) => {
      e.stopPropagation();
      toggleMenu("accountMenu", e.currentTarget);
    });
    document.addEventListener("click", (e) => {
      const accountMenu = byId("accountMenu");
      const target = asElement(e.target);
      if (accountMenu && target && !accountMenu.contains(target)) {
        closeAccountMenu();
      }
    });
    document.body.addEventListener("click", (e) => {
      const target = asElement(e.target);
      if (!target) return;
      const yearToggle = target.closest(".year-toggle");
      if (yearToggle) {
        e.preventDefault();
        const group = yearToggle.closest(".pay-year-group");
        if (!group) return;
        const next = !isYearGroupExpanded(group);
        setYearGroupExpanded(group, next);
        rememberYearExpanded(group.dataset.year, next);
        return;
      }
      const noteBtn = target.closest(".note-btn");
      if (noteBtn) {
        e.preventDefault();
        openNotePopover(noteBtn);
        return;
      }
      const improvementAddBtn = target.closest(
        ".improvement-add-btn"
      );
      if (improvementAddBtn) {
        e.preventDefault();
        openImprovementPopover(improvementAddBtn, "add");
        return;
      }
      const improvementEditBtn = target.closest(
        ".improvement-edit-btn"
      );
      if (improvementEditBtn) {
        e.preventDefault();
        openImprovementPopover(improvementEditBtn, "edit");
        return;
      }
      const grainBtn = target.closest(
        "#panel-chart .seg-toggle [data-grain]"
      );
      if (grainBtn) {
        e.preventDefault();
        setChartGrain(grainBtn.dataset.grain);
        return;
      }
      const tab = target.closest(".tab[data-tab]");
      if (tab) {
        e.preventDefault();
        activateTab(tab.dataset.tab);
      }
    });
    document.body.addEventListener("dblclick", (e) => {
      const target = asElement(e.target);
      if (!target) return;
      if (target.closest(".year-toggle")) {
        e.preventDefault();
        return;
      }
      const yearSummary = target.closest("tr.year-summary");
      if (yearSummary) {
        e.preventDefault();
        const group = yearSummary.closest(".pay-year-group");
        if (!group) return;
        const next = !isYearGroupExpanded(group);
        setYearGroupExpanded(group, next);
        rememberYearExpanded(group.dataset.year, next);
        return;
      }
      const payYear = target.closest("summary.pay-year");
      if (payYear) {
        e.preventDefault();
        const group = payYear.closest(
          "details.pay-year-group"
        );
        if (!group) return;
        const next = !isYearGroupExpanded(group);
        setYearGroupExpanded(group, next);
        rememberYearExpanded(group.dataset.year, next);
      }
    });
    document.body.addEventListener(
      "toggle",
      (e) => {
        const target = asElement(e.target);
        if (!target) return;
        const group = target.closest?.("details.pay-year-group");
        if (!(group instanceof HTMLDetailsElement) || target !== group) return;
        const expanded = group.open;
        group.dataset.expanded = expanded ? "true" : "false";
        rememberYearExpanded(group.dataset.year, expanded);
        const year = group.dataset.year;
        if (!year) return;
        const panel = panelEl("payments");
        panel?.querySelectorAll(
          `tbody.pay-year-group[data-year="${year}"]`
        ).forEach((other) => {
          setYearGroupExpanded(other, expanded);
        });
      },
      true
    );
    document.body.addEventListener("change", (e) => {
      const t = e.target;
      if (!(t instanceof HTMLElement)) return;
      const form = t.closest("#extraForm");
      if (!form) return;
      if (t.matches('input[name="recast"], input[name="recurring"]')) {
        syncExtraFormOptions(form);
      }
    });
    const queryTab = new URLSearchParams(location.search).get("tab");
    const legacyHashTab = location.hash.replace(/^#/, "");
    const dash = dashboard();
    const initialTab = normalizeTab(
      queryTab || legacyHashTab || dash?.dataset.tab || "calendar"
    );
    TAB_IDS.forEach((id) => {
      const panel = panelEl(id);
      if (panel) panel.dataset.stale = "false";
    });
    applyPaymentYearCollapse();
    bindPaymentsActionsPopover();
    activateTab(initialTab, { focus: false, syncUrl: true });
  }
  function installGlobalListeners() {
    document.body.addEventListener("markPanelsStale", (e) => {
      markPanelsStale(e.detail || {});
    });
    document.body.addEventListener("htmx:beforeSwap", (e) => {
      const targetId = e.detail.target?.id;
      if (targetId === "panel-chart" || targetId === "main-panel") {
        destroyBreakdownChart();
      }
    });
    document.body.addEventListener("htmx:afterSwap", (e) => {
      const targetId = e.detail.target.id;
      if (targetId === "panel-chart") {
        e.detail.target.dataset.stale = "false";
        renderBreakdownChart();
        syncDashboardMetaFromDom();
        return;
      }
      if (targetId === "panel-summary") {
        e.detail.target.dataset.stale = "false";
        syncDashboardMetaFromDom();
        return;
      }
      if (targetId === "panel-payments") {
        e.detail.target.dataset.stale = "false";
        syncDashboardMetaFromDom();
        bindPaymentsActionsPopover(e.detail.target);
        closePaymentsActionsPopover();
        return;
      }
      if (targetId === "panel-improvements") {
        e.detail.target.dataset.stale = "false";
        const path = e.detail.pathInfo?.requestPath || e.detail.elt?.getAttribute?.("hx-post") || "";
        if (/\/improvements\/?$/.test(path)) {
          requestAnimationFrame(() => {
            window.scrollTo({
              top: document.documentElement.scrollHeight,
              behavior: "smooth"
            });
          });
        }
        return;
      }
      if (targetId === "panel-calendar") {
        e.detail.target.dataset.stale = "false";
        syncDashboardMetaFromDom();
        return;
      }
      if (targetId === "main-panel") {
        syncProfileBarFromDashboard();
        TAB_IDS.forEach((id) => {
          const panel = panelEl(id);
          if (panel) panel.dataset.stale = "false";
        });
        const dash = dashboard();
        const queryTab = new URLSearchParams(location.search).get("tab");
        const nextTab = normalizeTab(queryTab || dash?.dataset.tab || "calendar");
        activateTab(nextTab, { focus: false, syncUrl: true });
      }
    });
    document.body.addEventListener("htmx:afterSettle", (e) => {
      const targetId = e.detail.target?.id;
      if (targetId === "panel-payments") {
        applyPaymentYearCollapse(e.detail.target);
        bindPaymentsActionsPopover(e.detail.target);
        requestAnimationFrame(() => restorePaymentsScroll(e.detail.target));
      } else if (targetId === "main-panel") {
        applyPaymentYearCollapse();
        bindPaymentsActionsPopover();
      }
    });
  }
  function boot() {
    installGlobalListeners();
    if (document.readyState === "loading") {
      document.addEventListener("DOMContentLoaded", bindUi);
    } else {
      bindUi();
    }
  }
  boot();
})();
