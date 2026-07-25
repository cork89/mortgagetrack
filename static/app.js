// Client helpers: popover, tabs, chart tooltip, profile menu, panel cache
(() => {
  const TAB_IDS = ["calendar", "payments", "chart"];

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

  function partialUrl(id) {
    const dash = dashboard();
    const year = dash?.dataset.year || String(new Date().getFullYear());
    const filter = dash?.dataset.filter || "all";
    const grain = dash?.dataset.grain || "monthly";
    const urls = {
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
      url.hash = id;
      const next = `${url.pathname}${url.search}${url.hash}`;
      if (`${location.pathname}${location.search}${location.hash}` !== next) {
        history.replaceState(null, "", next);
      }
    }
    refreshPanelIfStale(id);
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

  function openCreate() {
    closeProfileMenu();
    const form = document.getElementById("loanForm");
    form.action = "/profiles";
    form.method = "post";
    document.getElementById("popoverTitle").textContent = "New profile";
    document.getElementById("buildBtn").textContent = "Create profile";
    document.getElementById("nameFieldWrap").classList.remove("hidden");
    document.getElementById("loanFields").classList.remove("hidden");
    document.getElementById("resetWrap").classList.add("hidden");
    document.getElementById("extrasEditor").classList.add("hidden");
    document.getElementById("profileName").value = "Profile";
    document.getElementById("principal").value = "400000";
    document.getElementById("rate").value = "6.5";
    document.getElementById("term").value = "30";
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
    const form = document.getElementById("loanForm");
    form.action = `/profiles/${id}`;
    form.method = "post";
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
    popover()?.showPopover();
  }

  function openRename() {
    closeProfileMenu();
    const dash = dashboard();
    const select = document.getElementById("profileSelect");
    const id = dash?.dataset.profileId || select?.value;
    if (!id) return;
    const form = document.getElementById("loanForm");
    form.action = `/profiles/${id}/rename`;
    form.method = "post";
    document.getElementById("popoverTitle").textContent = "Rename profile";
    document.getElementById("buildBtn").textContent = "Save name";
    document.getElementById("nameFieldWrap").classList.remove("hidden");
    document.getElementById("loanFields").classList.add("hidden");
    document.getElementById("resetWrap").classList.add("hidden");
    document.getElementById("extrasEditor").classList.add("hidden");
    document.getElementById("profileName").value =
      dash?.dataset.name || select?.selectedOptions?.[0]?.textContent || "";
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
    document.getElementById("newProfileBtn")?.addEventListener("click", openCreate);
    document.getElementById("editProfileBtn")?.addEventListener("click", openEdit);
    document.getElementById("renameProfileBtn")?.addEventListener("click", openRename);
    document.getElementById("emptyNewBtn")?.addEventListener("click", (e) => {
      if (e.currentTarget.dataset.emptyAction === "edit") openEdit();
      else openCreate();
    });
    document.getElementById("closePopoverBtn")?.addEventListener("click", () => {
      popover()?.hidePopover();
    });
    document.getElementById("deleteProfileBtn")?.addEventListener("click", () => {
      closeProfileMenu();
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
    const hashTab = location.hash.replace(/^#/, "");
    const queryTab = new URLSearchParams(location.search).get("tab");
    const dash = dashboard();
    // Prefer URL (hash, then ?tab=) over the server-rendered default so refresh keeps the active tab.
    const initialTab = normalizeTab(hashTab || queryTab || dash?.dataset.tab || "calendar");
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
      TAB_IDS.forEach((id) => {
        const panel = panelEl(id);
        if (panel) panel.dataset.stale = "false";
      });
      const dash = dashboard();
      if (dash?.dataset.tab) {
        activateTab(dash.dataset.tab, { focus: false, syncUrl: false });
      }
    }
  });
})();
