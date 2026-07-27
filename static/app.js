// Client helpers: popover, tabs, chart tooltip, profile menu, panel cache
(() => {
  const TAB_IDS = ["summary", "calendar", "payments", "chart"];

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
    if (id === "chart") wireChartTooltip();
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
  }

  function closeProfileMenu() {
    const menu = document.getElementById("profileMenu");
    const btn = document.getElementById("profileMenuBtn");
    if (menu) menu.classList.remove("open");
    if (btn) btn.setAttribute("aria-expanded", "false");
  }

  function popover() {
    return document.getElementById("loanPopover");
  }

  function notePopover() {
    return document.getElementById("notePopover");
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

  function loanFormAction(mode, profileId) {
    if (mode === "edit" && profileId) return `/profiles/${profileId}`;
    if (mode === "rename" && profileId) return `/profiles/${profileId}/rename`;
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
    if (btn) {
      btn.setAttribute("formaction", action);
      btn.setAttribute("formmethod", "post");
    }
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

  function syncOwnerMenu() {
    const owner = isActiveOwner();
    const bar = document.getElementById("profileBar");
    if (bar) bar.dataset.isOwner = owner ? "true" : "false";
    ["renameProfileBtn", "deleteProfileBtn"].forEach((id) => {
      const el = document.getElementById(id);
      if (!el) return;
      el.hidden = !owner;
      el.disabled = !owner || !document.getElementById("profileSelect")?.value;
    });
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
    closeProfileMenu();
    setLoanFormMode("create");
    document.getElementById("popoverTitle").textContent = "New profile";
    document.getElementById("buildBtn").textContent = "Create profile";
    document.getElementById("nameFieldWrap").classList.remove("hidden");
    document.getElementById("loanFields").classList.remove("hidden");
    document.getElementById("resetWrap").classList.add("hidden");
    document.getElementById("extrasEditor").classList.add("hidden");
    hideShareEditor();
    document.getElementById("profileName").value = nextProfileName();
    document.getElementById("principal").value = "400000";
    document.getElementById("rate").value = "6.5";
    document.getElementById("term").value = "30";
    const start = document.getElementById("startDate");
    if (start) start.value = start.dataset.default || start.value;
    document.getElementById("error").textContent = "";
    popover()?.showPopover();
    document.getElementById("profileName").focus();
  }

  function openEdit() {
    closeProfileMenu();
    const dash = dashboard();
    if (!dash) {
      openCreate();
      return;
    }
    const id = dash.dataset.profileId;
    setLoanFormMode("edit", id);
    document.getElementById("popoverTitle").textContent = `Edit ${dash.dataset.name || "profile"}`;
    document.getElementById("buildBtn").textContent = "Save changes";
    document.getElementById("nameFieldWrap").classList.remove("hidden");
    document.getElementById("loanFields").classList.remove("hidden");
    document.getElementById("resetWrap").classList.remove("hidden");
    document.getElementById("extrasEditor").classList.remove("hidden");
    document.getElementById("profileName").value = dash.dataset.name || "";
    document.getElementById("principal").value = dash.dataset.principal || "400000";
    document.getElementById("rate").value = dash.dataset.rate || "6.5";
    document.getElementById("term").value = dash.dataset.term || "30";
    document.getElementById("startDate").value = dash.dataset.start || "";
    document.getElementById("error").textContent = "";
    document.getElementById("resetPaidBtn").onclick = () => {
      if (!confirm("Clear all tracked payments for this profile?")) return;
      const f = document.createElement("form");
      f.method = "post";
      f.action = `/profiles/${id}/clear-paid`;
      document.body.appendChild(f);
      f.submit();
    };
    loadSharePanel(id);
    popover()?.showPopover();
  }

  function openRename() {
    closeProfileMenu();
    if (!isActiveOwner()) return;
    const dash = dashboard();
    const select = document.getElementById("profileSelect");
    const id = dash?.dataset.profileId || select?.value;
    if (!id) return;
    setLoanFormMode("rename", id);
    document.getElementById("popoverTitle").textContent = "Rename profile";
    document.getElementById("buildBtn").textContent = "Save name";
    document.getElementById("nameFieldWrap").classList.remove("hidden");
    document.getElementById("loanFields").classList.add("hidden");
    document.getElementById("resetWrap").classList.add("hidden");
    document.getElementById("extrasEditor").classList.add("hidden");
    hideShareEditor();
    const label = select?.selectedOptions?.[0]?.textContent || "";
    document.getElementById("profileName").value =
      dash?.dataset.name || label.replace(/\s*\(shared\)\s*$/, "") || "";
    document.getElementById("error").textContent = "";
    popover()?.showPopover();
    document.getElementById("profileName").focus();
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
    panel.querySelectorAll("[data-grain-svg]").forEach((el) => {
      el.classList.toggle("hidden", el.dataset.grainSvg !== next);
    });
    const dash = dashboard();
    if (dash) dash.dataset.grain = next;
    const tip = document.getElementById("chartTooltip");
    tip?.classList.remove("visible");
    wireChartTooltip();
  }

  function wireChartTooltip() {
    const wrap = document.getElementById("chartWrap");
    const tip = document.getElementById("chartTooltip");
    const svg = document.querySelector("#panel-chart [data-grain-svg]:not(.hidden)");
    if (!wrap || !tip || !svg) return;
    let buckets = [];
    try {
      buckets = JSON.parse(svg.dataset.buckets || "[]");
    } catch {
      buckets = [];
    }
    wrap.onmousemove = (e) => {
      const activeSvg = document.querySelector("#panel-chart [data-grain-svg]:not(.hidden)");
      if (!activeSvg || !activeSvg.contains(e.target)) {
        tip.classList.remove("visible");
        return;
      }
      const hit = e.target.closest("[data-idx]");
      if (!hit) {
        tip.classList.remove("visible");
        return;
      }
      let activeBuckets = buckets;
      try {
        activeBuckets = JSON.parse(activeSvg.dataset.buckets || "[]");
      } catch {
        activeBuckets = buckets;
      }
      const bucket = activeBuckets[Number(hit.dataset.idx)];
      if (!bucket) return;
      const countLine =
        bucket.count != null
          ? `<div class="row"><span>Payments</span><span>${bucket.count}</span></div>`
          : "";
      tip.innerHTML = `
        <strong>${bucket.label}</strong>
        <div class="row"><span>Principal</span><span>${money(bucket.principal)}</span></div>
        <div class="row"><span>Interest</span><span>${money(bucket.interest)}</span></div>
        <div class="row"><span>Payment</span><span>${money(bucket.payment)}</span></div>
        ${countLine}`;
      const rect = wrap.getBoundingClientRect();
      tip.style.left = `${e.clientX - rect.left}px`;
      tip.style.top = `${e.clientY - rect.top}px`;
      tip.classList.add("visible");
    };
    wrap.onmouseleave = () => tip.classList.remove("visible");
  }

  function bindUi() {
    document.getElementById("loanForm")?.addEventListener("submit", (e) => {
      const form = e.currentTarget;
      const mode = form.dataset.mode || "create";
      const profileId = form.dataset.profileId;
      if ((mode === "edit" || mode === "rename") && !profileId) {
        e.preventDefault();
        document.getElementById("error").textContent = "No profile selected.";
        return;
      }
      // Re-assert destination so a stale action from a prior edit/rename
      // cannot turn "Create profile" into an update of the active profile.
      setLoanFormMode(mode, profileId);
    });
    document.getElementById("newProfileBtn")?.addEventListener("click", openCreate);
    document.getElementById("editProfileBtn")?.addEventListener("click", openEdit);
    document.getElementById("renameProfileBtn")?.addEventListener("click", openRename);
    document.getElementById("profileSelect")?.addEventListener("change", syncOwnerMenu);
    syncOwnerMenu();
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
    document.getElementById("deleteProfileBtn")?.addEventListener("click", () => {
      closeProfileMenu();
      if (!isActiveOwner()) return;
      const select = document.getElementById("profileSelect");
      const id = select?.value;
      const name = select?.selectedOptions?.[0]?.textContent || "this profile";
      if (!id) return;
      if (!confirm(`Delete profile “${name}”? This cannot be undone.`)) return;
      const f = document.createElement("form");
      f.method = "post";
      f.action = `/profiles/${id}/delete`;
      document.body.appendChild(f);
      f.submit();
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
    document.getElementById("profileMenuBtn")?.addEventListener("click", (e) => {
      e.stopPropagation();
      const menu = document.getElementById("profileMenu");
      const open = !menu.classList.contains("open");
      menu.classList.toggle("open", open);
      e.currentTarget.setAttribute("aria-expanded", open ? "true" : "false");
    });
    document.addEventListener("click", (e) => {
      const menu = document.getElementById("profileMenu");
      if (menu && !menu.contains(e.target)) closeProfileMenu();
    });
    document.body.addEventListener("click", (e) => {
      const noteBtn = e.target.closest(".note-btn");
      if (noteBtn) {
        e.preventDefault();
        openNotePopover(noteBtn);
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
    wireChartTooltip();
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
    markPanelsStale(e.detail || {});
  });

  document.body.addEventListener("htmx:afterSwap", (e) => {
    const targetId = e.detail.target.id;
    if (targetId === "panel-chart") {
      e.detail.target.dataset.stale = "false";
      wireChartTooltip();
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
    if (targetId === "panel-calendar") {
      e.detail.target.dataset.stale = "false";
      syncDashboardMetaFromDom();
      return;
    }
    if (targetId === "main-panel") {
      wireChartTooltip();
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
