// Client helpers: popover, tabs, chart tooltip, profile menu
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
    const panel = document.getElementById(`panel-${id}`);
    if (panel) panel.classList.add("active");
    const dash = document.getElementById("dashboard");
    if (dash) dash.dataset.tab = id;
    if (focus) target.focus();
    if (syncUrl) {
      const hash = `#${id}`;
      if (location.hash !== hash) history.replaceState(null, "", hash);
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

  function openCreate() {
    closeProfileMenu();
    const form = document.getElementById("loanForm");
    const dash = document.getElementById("dashboard");
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
    const dash = document.getElementById("dashboard");
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
    const dash = document.getElementById("dashboard");
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

  function wireChartTooltip() {
    const svg = document.getElementById("chartSvg");
    const tip = document.getElementById("chartTooltip");
    const wrap = document.getElementById("chartWrap");
    if (!svg || !tip || !wrap) return;
    let buckets = [];
    try {
      buckets = JSON.parse(svg.dataset.buckets || "[]");
    } catch {
      buckets = [];
    }
    svg.onmousemove = (e) => {
      const hit = e.target.closest("[data-idx]");
      if (!hit) {
        tip.classList.remove("visible");
        return;
      }
      const bucket = buckets[Number(hit.dataset.idx)];
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
    svg.onmouseleave = () => tip.classList.remove("visible");
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
      const tab = e.target.closest(".tab[data-tab]");
      if (tab) {
        e.preventDefault();
        activateTab(tab.dataset.tab);
      }
    });
    wireChartTooltip();
    const hashTab = normalizeTab(location.hash.replace(/^#/, ""));
    const dash = document.getElementById("dashboard");
    activateTab(dash?.dataset.tab || hashTab, { focus: false, syncUrl: false });
  }

  document.addEventListener("DOMContentLoaded", bindUi);
  document.body.addEventListener("htmx:afterSwap", (e) => {
    const targetId = e.detail.target.id;
    if (targetId === "panel-chart") {
      wireChartTooltip();
      activateTab("chart", { focus: false, syncUrl: true });
      return;
    }
    if (targetId === "panel-payments") {
      activateTab("payments", { focus: false, syncUrl: true });
      return;
    }
    if (targetId === "panel-calendar") {
      activateTab("calendar", { focus: false, syncUrl: true });
      return;
    }
    if (targetId === "main-panel") {
      wireChartTooltip();
      const dash = document.getElementById("dashboard");
      if (dash?.dataset.tab) {
        activateTab(dash.dataset.tab, { focus: false, syncUrl: false });
      }
    }
  });
})();
