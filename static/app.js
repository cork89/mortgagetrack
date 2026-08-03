// Client helpers: popover, tabs, breakdown chart, profile actions, panel cache
(() => {
  const TAB_IDS = ["summary", "calendar", "payments", "improvements", "chart"];
  let breakdownChart = null;

  function csrfToken() {
    return document.querySelector('meta[name="csrf-token"]')?.content || "";
  }

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
      let input = form.querySelector('input[name="csrf_token"]');
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

  function money(n) {
    return Number(n).toLocaleString(undefined, {
      style: "currency",
      currency: "USD",
      maximumFractionDigits: 2,
    });
  }

  function normalizeTab(tab) {
    return TAB_IDS.includes(tab) ? tab : "calendar";
  }

  function dashboard() {
    return document.getElementById("dashboard");
  }

  function panelEl(id) {
    return document.getElementById(`panel-${id}`);
  }

  function syncDashboardMetaFromDom() {
    const dash = dashboard();
    if (!dash) return;
    const yearLabel = document.querySelector("#panel-calendar .month-label");
    if (yearLabel?.textContent?.trim()) {
      dash.dataset.year = yearLabel.textContent.trim();
    }
    const filter = document.querySelector("#panel-payments select[name='filter']");
    if (filter?.value) {
      dash.dataset.filter = filter.value;
    }
    const grainBtn = document.querySelector("#panel-chart [data-grain].active");
    if (grainBtn?.dataset.grain) {
      dash.dataset.grain = grainBtn.dataset.grain;
    }
  }

  function syncProfileBarFromDashboard() {
    const meta = document.getElementById("profileMeta");
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
    const year = dash?.dataset.year || String(new Date().getFullYear());
    const filter = dash?.dataset.filter || "all";
    const grain = dash?.dataset.grain || "monthly";
    const urls = {
      summary: `/partials/summary?tab=summary&year=${encodeURIComponent(year)}&filter=${encodeURIComponent(filter)}&grain=${encodeURIComponent(grain)}`,
      calendar: `/partials/calendar?year=${encodeURIComponent(year)}&tab=calendar&filter=${encodeURIComponent(filter)}&grain=${encodeURIComponent(grain)}`,
      payments: `/partials/payments?tab=payments&filter=${encodeURIComponent(filter)}&year=${encodeURIComponent(year)}&grain=${encodeURIComponent(grain)}`,
      improvements: `/partials/improvements?tab=improvements&year=${encodeURIComponent(year)}&filter=${encodeURIComponent(filter)}&grain=${encodeURIComponent(grain)}`,
      chart: `/partials/chart?tab=chart&grain=${encodeURIComponent(grain)}&year=${encodeURIComponent(year)}&filter=${encodeURIComponent(filter)}`,
    };
    return urls[id];
  }

  function markPanelsStale({ keep, invalidateChart }) {
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
    if (id === "chart") renderBreakdownChart();
  }

  let scrolledToCurrentPayment = false;

  function maybeScrollToCurrentMonthPayment() {
    if (scrolledToCurrentPayment) return;
    const panel = panelEl("payments");
    if (!panel?.classList.contains("active")) return;
    const target = [...panel.querySelectorAll(".payment-current-month")].find(
      (el) => el.getClientRects().length > 0
    );
    scrolledToCurrentPayment = true;
    if (!target) return;
    requestAnimationFrame(() => {
      target.scrollIntoView({ block: "center", behavior: "smooth" });
    });
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
      Promise.resolve(refresh).finally(() => maybeScrollToCurrentMonthPayment());
    }
    if (id === "chart") {
      Promise.resolve(refresh).finally(() => renderBreakdownChart());
    }
  }

  function closeMenu(menuId, btnId) {
    const menu = document.getElementById(menuId);
    const btn = document.getElementById(btnId);
    if (menu) menu.classList.remove("open");
    if (btn) btn.setAttribute("aria-expanded", "false");
  }

  function closeAccountMenu() {
    closeMenu("accountMenu", "accountMenuBtn");
  }

  function toggleMenu(menuId, btn) {
    const menu = document.getElementById(menuId);
    if (!menu || !btn) return;
    const open = !menu.classList.contains("open");
    menu.classList.toggle("open", open);
    btn.setAttribute("aria-expanded", open ? "true" : "false");
  }

  function popover() {
    return document.getElementById("loanPopover");
  }

  function notePopover() {
    return document.getElementById("notePopover");
  }

  function dashboardVersion() {
    return dashboard()?.dataset.version || "";
  }

  function syncDashboardVersion(version) {
    if (version == null || version === "") return;
    const next = String(version);
    const dash = dashboard();
    if (dash) dash.dataset.version = next;
    const profileVersion = document.getElementById("profileVersion");
    if (profileVersion) profileVersion.value = next;
    const noteVersion = document.getElementById("noteVersion");
    if (noteVersion) noteVersion.value = next;
    const improvementVersion = document.getElementById("improvementVersion");
    if (improvementVersion) improvementVersion.value = next;
  }

  function isProfileWritePath(path) {
    if (!path || !path.startsWith("/profiles/")) return false;
    if (path === "/profiles" || path === "/profiles/switch") return false;
    if (path.includes("/share") || path.includes("/leave") || path.includes("/collaborators")) {
      return false;
    }
    if (/\/profiles\/[^/]+\/delete$/.test(path)) return false;
    return true;
  }

  function showConflictToast(message) {
    let toast = document.getElementById("conflictToast");
    if (!toast) {
      toast = document.createElement("div");
      toast.id = "conflictToast";
      toast.className = "conflict-toast";
      toast.setAttribute("role", "status");
      document.body.appendChild(toast);
    }
    toast.textContent = message || "Someone else updated this profile. Your view has been refreshed.";
    toast.classList.add("visible");
    clearTimeout(showConflictToast._timer);
    showConflictToast._timer = setTimeout(() => toast.classList.remove("visible"), 6000);
  }

  function openNotePopover(btn) {
    const dash = dashboard();
    const profileId = dash?.dataset.profileId;
    if (!profileId) return;

    let note = "";
    try {
      note = JSON.parse(btn.getAttribute("data-note-json") || '""');
    } catch {
      note = "";
    }

    const form = document.getElementById("noteForm");
    form.action = `/profiles/${profileId}/notes`;
    form.setAttribute("hx-post", `/profiles/${profileId}/notes`);
    form.setAttribute("hx-target", "#panel-payments");
    form.setAttribute("hx-swap", "innerHTML show:none");

    document.getElementById("notePayKey").value = btn.dataset.payKey || "";
    document.getElementById("noteFilter").value = dash.dataset.filter || "all";
    document.getElementById("noteYear").value = dash.dataset.year || String(new Date().getFullYear());
    document.getElementById("noteGrain").value = dash.dataset.grain || "monthly";
    document.getElementById("noteVersion").value = dash.dataset.version || "";
    document.getElementById("noteText").value = note;
    document.getElementById("notePopoverDue").textContent = btn.dataset.due
      ? `Due ${btn.dataset.due}`
      : "";
    document.getElementById("notePopoverTitle").textContent = note.trim()
      ? "Edit note"
      : "Add note";

    if (typeof htmx !== "undefined") htmx.process(form);
    notePopover()?.showPopover();
    document.getElementById("noteText")?.focus();
  }

  function closeNotePopover() {
    notePopover()?.hidePopover();
  }

  function improvementPopover() {
    return document.getElementById("improvementPopover");
  }

  function parseJsonAttr(el, name) {
    try {
      return JSON.parse(el.getAttribute(name) || '""');
    } catch {
      return "";
    }
  }

  function openImprovementPopover(btn) {
    const dash = dashboard();
    const profileId = dash?.dataset.profileId;
    const improvementId = btn.dataset.id;
    if (!profileId || !improvementId) return;

    const form = document.getElementById("improvementForm");
    const action = `/profiles/${profileId}/improvements/${improvementId}/update`;
    form.action = action;
    form.setAttribute("hx-post", action);
    form.setAttribute("hx-target", "#panel-improvements");
    form.setAttribute("hx-swap", "innerHTML show:none");

    document.getElementById("improvementVersion").value = dash.dataset.version || "";
    document.getElementById("improvementDate").value = btn.dataset.date || "";
    document.getElementById("improvementAmount").value = btn.dataset.amount || "";
    document.getElementById("improvementNote").value = parseJsonAttr(btn, "data-note-json");
    document.getElementById("improvementDetail").value = parseJsonAttr(btn, "data-detail-json");

    if (typeof htmx !== "undefined") htmx.process(form);
    improvementPopover()?.showPopover();
    document.getElementById("improvementNote")?.focus();
  }

  function closeImprovementPopover() {
    improvementPopover()?.hidePopover();
  }

  function loanFormAction(mode, profileId) {
    if (mode === "edit" && profileId) return `/profiles/${profileId}`;
    return "/profiles";
  }

  function setLoanFormMode(mode, profileId) {
    const form = document.getElementById("loanForm");
    const btn = document.getElementById("buildBtn");
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
    const select = document.getElementById("profileSelect");
    const count = [...(select?.options || [])].filter((o) => o.value).length;
    return `Profile ${count + 1}`;
  }

  function isActiveOwner() {
    const select = document.getElementById("profileSelect");
    const opt = select?.selectedOptions?.[0];
    if (opt?.dataset.shared != null) return opt.dataset.shared !== "true";
    const bar = document.getElementById("profileBar");
    return bar?.dataset.isOwner !== "false";
  }

  function syncOwnerState() {
    const owner = isActiveOwner();
    const bar = document.getElementById("profileBar");
    if (bar) bar.dataset.isOwner = owner ? "true" : "false";
  }

  function hideShareEditor() {
    const wrap = document.getElementById("shareEditor");
    const panel = document.getElementById("sharePanel");
    wrap?.classList.add("hidden");
    if (panel) panel.innerHTML = "";
  }

  function fillShareInviteUrl() {
    const input = document.getElementById("shareInviteUrl");
    if (!input) return;
    const path = input.dataset.path || "";
    if (path) input.value = `${window.location.origin}${path}`;
  }

  function loadSharePanel(profileId) {
    const wrap = document.getElementById("shareEditor");
    const panel = document.getElementById("sharePanel");
    if (!wrap || !panel || !profileId) return;
    wrap.classList.remove("hidden");
    if (typeof htmx === "undefined") return;
    htmx.ajax("GET", `/profiles/${profileId}/share-panel`, {
      target: "#sharePanel",
      swap: "innerHTML",
    });
  }

  function openCreate() {
    setLoanFormMode("create");
    document.getElementById("popoverTitle").textContent = "New profile";
    document.getElementById("buildBtn").textContent = "Create profile";
    document.getElementById("nameFieldWrap").classList.remove("hidden");
    document.getElementById("loanFields").classList.remove("hidden");
    document.getElementById("resetWrap").classList.add("hidden");
    document.getElementById("deleteWrap")?.classList.add("hidden");
    hideShareEditor();
    document.getElementById("profileName").value = nextProfileName();
    document.getElementById("principal").value = "400000";
    document.getElementById("rate").value = "6.5";
    document.getElementById("term").value = "30";
    const start = document.getElementById("startDate");
    if (start) start.value = start.dataset.default || start.value;
    document.getElementById("error").textContent = "";
    const versionInput = document.getElementById("profileVersion");
    if (versionInput) versionInput.value = "";
    popover()?.showPopover();
    document.getElementById("profileName").focus();
  }

  function openEdit() {
    const dash = dashboard();
    if (!dash) {
      openCreate();
      return;
    }
    const id = dash.dataset.profileId;
    setLoanFormMode("edit", id);
    document.getElementById("popoverTitle").textContent = "Edit profile";
    document.getElementById("buildBtn").textContent = "Save changes";
    document.getElementById("nameFieldWrap").classList.remove("hidden");
    document.getElementById("loanFields").classList.remove("hidden");
    document.getElementById("resetWrap").classList.remove("hidden");
    document.getElementById("deleteWrap")?.classList.toggle("hidden", !isActiveOwner());
    document.getElementById("profileName").value = dash.dataset.name || "";
    document.getElementById("principal").value = dash.dataset.principal || "400000";
    document.getElementById("rate").value = dash.dataset.rate || "6.5";
    document.getElementById("term").value = dash.dataset.term || "30";
    document.getElementById("startDate").value = dash.dataset.start || "";
    document.getElementById("error").textContent = "";
    const versionInput = document.getElementById("profileVersion");
    if (versionInput) versionInput.value = dash.dataset.version || "";
    document.getElementById("resetPaidBtn").onclick = () => {
      if (!confirm("Clear all tracked payments for this profile?")) return;
      const version = dashboardVersion();
      if (!version || typeof htmx === "undefined") return;
      htmx.ajax("POST", `/profiles/${id}/clear-paid`, {
        values: { version },
        headers: { "HX-Request": "true" },
      });
    };
    loadSharePanel(id);
    popover()?.showPopover();
  }

  function chartGrain() {
    return dashboard()?.dataset.grain === "yearly" ? "yearly" : "monthly";
  }

  function cssVar(name) {
    return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  }

  function parseBuckets(raw) {
    try {
      const data = JSON.parse(raw || "[]");
      return Array.isArray(data) ? data : [];
    } catch {
      return [];
    }
  }

  function destroyBreakdownChart() {
    if (!breakdownChart) return;
    try {
      breakdownChart.destroy();
    } catch {
      /* already gone with the DOM */
    }
    breakdownChart = null;
  }

  function breakdownTooltipHtml(params) {
    const datum = params?.datum ?? params?.[0]?.datum;
    if (!datum) return "";
    const countRow =
      datum.count != null
        ? `<div class="row"><span>Payments</span><span>${datum.count}</span></div>`
        : "";
    return `
      <div class="chart-tooltip-card">
        <strong>${datum.label}</strong>
        <div class="row"><span>Principal</span><span>${money(datum.principal)}</span></div>
        <div class="row"><span>Interest</span><span>${money(datum.interest)}</span></div>
        <div class="row"><span>Payment</span><span>${money(datum.payment)}</span></div>
        ${countRow}
      </div>`;
  }

  // Category keys for x labels every 2 years (first bucket of that year).
  function yearAxisTickValues(data) {
    const firstLabelByYear = new Map();
    for (const row of data) {
      if (row?.year == null || firstLabelByYear.has(row.year)) continue;
      firstLabelByYear.set(row.year, row.label);
    }
    const years = [...firstLabelByYear.keys()].sort((a, b) => a - b);
    const ticks = [];
    for (let i = 0; i < years.length; i += 2) {
      ticks.push(firstLabelByYear.get(years[i]));
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
          tooltip: { renderer: breakdownTooltipHtml },
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
          tooltip: { renderer: breakdownTooltipHtml },
        },
      ],
      axes: {
        x: {
          type: "category",
          paddingInner: data.length > 90 ? 0.05 : 0.2,
          paddingOuter: 0.05,
          interval: yearTicks.length ? { values: yearTicks } : undefined,
          label: {
            color: inkSoft,
            fontFamily: fontBody,
            fontSize: 11,
            avoidCollisions: false,
            formatter: ({ value }) => {
              const year = yearByLabel.get(value);
              if (year != null) return String(year);
              const match = String(value).match(/(\d{4})\s*$/);
              return match ? match[1] : String(value);
            },
          },
          line: { enabled: false },
          tick: { enabled: false },
        },
        y: {
          type: "number",
          label: {
            color: inkSoft,
            fontFamily: fontBody,
            fontSize: 11,
            formatter: ({ value }) => money(value),
          },
          gridLine: {
            style: [{ stroke: cssVar("--line") || "rgba(28, 42, 46, 0.12)" }],
          },
          line: { enabled: false },
          tick: { enabled: false },
        },
      },
      legend: {
        position: "top",
        spacing: 12,
        item: {
          marker: { shape: "square", size: 10 },
          label: {
            color: inkSoft,
            fontFamily: fontBody,
            fontSize: 12,
          },
        },
      },
      // Avoid shared mode: each series renderer already returns the full bucket card,
      // and shared would concatenate both copies.
      tooltip: {
        mode: "single",
      },
    };
  }

  function renderBreakdownChart() {
    const panel = panelEl("chart");
    const wrap = document.getElementById("chartWrap");
    const container = document.getElementById("breakdownChart");
    if (!panel?.classList.contains("active") || !wrap || !container) return;
    if (typeof agCharts === "undefined" || !agCharts.AgCharts) return;

    const grain = chartGrain();
    const data = parseBuckets(
      grain === "yearly" ? wrap.dataset.yearlyBuckets : wrap.dataset.monthlyBuckets,
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

  function bindUi() {
    document.body.addEventListener("htmx:configRequest", (e) => {
      const path = e.detail.path || "";
      if (!isProfileWritePath(path) || e.detail.verb !== "post") return;
      const version = dashboardVersion();
      if (!version) return;
      e.detail.parameters = e.detail.parameters || {};
      e.detail.parameters.version = version;
    });
    document.body.addEventListener("htmx:beforeSwap", (e) => {
      if (e.detail.xhr?.status !== 409) return;
      e.detail.shouldSwap = true;
      e.detail.isError = false;
      const message = e.detail.xhr.getResponseHeader("X-Conflict-Message");
      showConflictToast(message);
      popover()?.hidePopover();
      closeNotePopover();
      closeImprovementPopover();
    });
    document.getElementById("loanForm")?.addEventListener("submit", (e) => {
      const form = e.currentTarget;
      const mode = form.dataset.mode || "create";
      const profileId = form.dataset.profileId;
      if (mode === "edit" && !profileId) {
        e.preventDefault();
        document.getElementById("error").textContent = "No profile selected.";
        return;
      }
      // Re-assert destination so a stale action from a prior edit
      // cannot turn "Create profile" into an update of the active profile.
      setLoanFormMode(mode, profileId);
    });
    document.getElementById("newProfileBtn")?.addEventListener("click", openCreate);
    document.getElementById("editProfileBtn")?.addEventListener("click", openEdit);
    document.getElementById("profileSelect")?.addEventListener("change", syncOwnerState);
    syncOwnerState();
    document.getElementById("emptyNewBtn")?.addEventListener("click", (e) => {
      if (e.currentTarget.dataset.emptyAction === "edit") openEdit();
      else openCreate();
    });
    document.getElementById("closePopoverBtn")?.addEventListener("click", () => {
      popover()?.hidePopover();
    });
    document.getElementById("closeNotePopoverBtn")?.addEventListener("click", closeNotePopover);
    document.getElementById("clearNoteBtn")?.addEventListener("click", () => {
      const text = document.getElementById("noteText");
      if (text) text.value = "";
      text?.focus();
    });
    document.getElementById("noteForm")?.addEventListener("htmx:afterRequest", (e) => {
      if (e.detail.successful) closeNotePopover();
    });
    document.getElementById("closeImprovementPopoverBtn")?.addEventListener("click", closeImprovementPopover);
    document.getElementById("cancelImprovementBtn")?.addEventListener("click", closeImprovementPopover);
    document.getElementById("improvementForm")?.addEventListener("htmx:afterRequest", (e) => {
      if (e.detail.successful) closeImprovementPopover();
    });
    document.getElementById("deleteProfileBtn")?.addEventListener("click", () => {
      if (!isActiveOwner()) return;
      const select = document.getElementById("profileSelect");
      const dash = dashboard();
      const id = dash?.dataset.profileId || select?.value;
      const name = dash?.dataset.name || select?.selectedOptions?.[0]?.textContent || "this profile";
      if (!id) return;
      if (!confirm(`Delete profile “${name}”? This cannot be undone.`)) return;
      if (typeof htmx === "undefined") return;
      popover()?.hidePopover();
      htmx.ajax("POST", `/profiles/${id}/delete`, {
        headers: { "HX-Request": "true" },
      });
    });
    document.getElementById("sharePanel")?.addEventListener("click", (e) => {
      const btn = e.target.closest("#copyShareLinkBtn");
      if (!btn) return;
      const input = document.getElementById("shareInviteUrl");
      fillShareInviteUrl();
      const value = input?.value || "";
      if (!value) return;
      navigator.clipboard?.writeText(value).then(() => {
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
    document.getElementById("accountMenuBtn")?.addEventListener("click", (e) => {
      e.stopPropagation();
      toggleMenu("accountMenu", e.currentTarget);
    });
    document.addEventListener("click", (e) => {
      const accountMenu = document.getElementById("accountMenu");
      if (accountMenu && !accountMenu.contains(e.target)) closeAccountMenu();
    });
    document.body.addEventListener("click", (e) => {
      const noteBtn = e.target.closest(".note-btn");
      if (noteBtn) {
        e.preventDefault();
        openNotePopover(noteBtn);
        return;
      }
      const improvementEditBtn = e.target.closest(".improvement-edit-btn");
      if (improvementEditBtn) {
        e.preventDefault();
        openImprovementPopover(improvementEditBtn);
        return;
      }
      const grainBtn = e.target.closest("#panel-chart .seg-toggle [data-grain]");
      if (grainBtn) {
        e.preventDefault();
        setChartGrain(grainBtn.dataset.grain);
        return;
      }
      const tab = e.target.closest(".tab[data-tab]");
      if (tab) {
        e.preventDefault();
        activateTab(tab.dataset.tab);
      }
    });
    const queryTab = new URLSearchParams(location.search).get("tab");
    // One-time migrate old #tab bookmarks to ?tab=.
    const legacyHashTab = location.hash.replace(/^#/, "");
    const dash = dashboard();
    // Prefer ?tab= over the server-rendered default so refresh keeps the active tab.
    const initialTab = normalizeTab(queryTab || legacyHashTab || dash?.dataset.tab || "calendar");
    TAB_IDS.forEach((id) => {
      const panel = panelEl(id);
      if (panel) panel.dataset.stale = "false";
    });
    activateTab(initialTab, { focus: false, syncUrl: true });
  }

  document.addEventListener("DOMContentLoaded", bindUi);

  document.body.addEventListener("markPanelsStale", (e) => {
    const detail = e.detail || {};
    if (detail.version != null) syncDashboardVersion(detail.version);
    markPanelsStale(detail);
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
      return;
    }
    if (targetId === "panel-payments") {
      e.detail.target.dataset.stale = "false";
      syncDashboardMetaFromDom();
      return;
    }
    if (targetId === "panel-improvements") {
      e.detail.target.dataset.stale = "false";
      const path = e.detail.pathInfo?.requestPath || e.detail.elt?.getAttribute?.("hx-post") || "";
      if (/\/improvements\/?$/.test(path)) {
        requestAnimationFrame(() => {
          window.scrollTo({ top: document.documentElement.scrollHeight, behavior: "smooth" });
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
})();
