import {
  destroyBreakdownChart,
  renderBreakdownChart,
  setChartGrain,
} from "./chart";
import {
  TAB_IDS,
  activateTab,
  asElement,
  byId,
  closeAccountMenu,
  dashboard,
  installCoreListeners,
  markPanelsStale,
  normalizeTab,
  panelEl,
  setTabActivationHooks,
  syncDashboardMetaFromDom,
  syncProfileBarFromDashboard,
  toggleMenu,
} from "./core";
import {
  applyPaymentYearCollapse,
  bindPaymentsActionsPopover,
  closeNotePopover,
  closePaymentsActionsPopover,
  installPaymentsListeners,
  isPaidToggle,
  isYearGroupExpanded,
  maybeScrollToCurrentMonthPayment,
  openNotePopover,
  rememberYearExpanded,
  restorePaymentsScroll,
  setYearGroupExpanded,
  syncExtraFormOptions,
} from "./payments";
import {
  canCreateProfile,
  closeImprovementPopover,
  fillShareInviteUrl,
  isOptionOwner,
  openCreate,
  openEdit,
  openImprovementPopover,
  openProfileManager,
  openSelectedProfile,
  popover,
  postProfileJson,
  prepareProfileManager,
  profileOptionById,
  refreshDashboardForActive,
  renderProfileGutter,
  selectCreateMode,
  selectProfile,
  setCanCreateProfile,
  setLoanFormMode,
  showLoanPopover,
  showProfileManagerError,
  syncOwnerState,
  syncProfileSelect,
  upsertProfileOption,
  type CopyProfileData,
  type DeleteProfileData,
  type LoanFormMode,
} from "./profiles";

setTabActivationHooks({
  onPaymentsActivated: maybeScrollToCurrentMonthPayment,
  onChartActivated: renderBreakdownChart,
});

installCoreListeners();
installPaymentsListeners();

function bindUi(): void {
  document.body.addEventListener("htmx:configRequest", (e) => {
    const elt = e.detail.elt;
    if (!isPaidToggle(elt) || e.detail.verb !== "post") return;
    // Desired state is the opposite of what's currently shown (before optimistic flip).
    e.detail.parameters = e.detail.parameters || {};
    e.detail.parameters.paid = !elt.classList.contains("paid");
  });
  byId<HTMLFormElement>("loanForm")?.addEventListener("submit", (e) => {
    const form = e.currentTarget as HTMLFormElement;
    const mode = (form.dataset.mode || "create") as LoanFormMode;
    const profileId = form.dataset.profileId;
    if (mode === "edit" && !profileId) {
      e.preventDefault();
      const error = byId("error");
      if (error) error.textContent = "No profile selected.";
      return;
    }
    // Re-assert destination so a stale action from a prior edit
    // cannot turn "Create profile" into an update of the active profile.
    setLoanFormMode(mode, profileId);
  });
  // Prepare content, then show on the next frame so the opening click doesn't
  // immediately light-dismiss the popover. `popovertarget` (when present) also
  // associates the invoker for accessibility.
  byId("manageProfilesBtn")?.addEventListener("click", () => {
    prepareProfileManager({ mode: "edit" });
    showLoanPopover();
  });
  byId("profileGutterNewBtn")?.addEventListener("click", () => {
    selectCreateMode();
    byId<HTMLInputElement>("profileName")?.focus();
  });
  byId("profileGutterList")?.addEventListener("click", (e) => {
    const btn = asElement(e.target)?.closest<HTMLElement>(
      ".profile-gutter-item",
    );
    if (!btn) return;
    const id = btn.dataset.profileId;
    if (id) selectProfile(id);
  });
  byId("openProfileBtn")?.addEventListener("click", openSelectedProfile);
  byId("resetPaidBtn")?.addEventListener("click", () => {
    const form = byId<HTMLFormElement>("loanForm");
    const id = form?.dataset.profileId;
    if (!id || form.dataset.mode !== "edit") return;
    if (!confirm("Clear all tracked payments for this profile?")) return;
    if (typeof htmx === "undefined") return;
    htmx.ajax("POST", `/profiles/${id}/clear-paid`, {
      headers: { "HX-Request": "true" },
    });
  });
  byId("profileSelect")?.addEventListener("change", syncOwnerState);
  syncOwnerState();
  byId("emptyNewBtn")?.addEventListener("click", (e) => {
    const btn = e.currentTarget as HTMLElement;
    if (btn.dataset.emptyAction === "edit") openEdit();
    else openCreate();
  });
  byId("closePopoverBtn")?.addEventListener("click", () => {
    popover()?.hidePopover();
  });
  byId("closeNotePopoverBtn")?.addEventListener("click", closeNotePopover);
  byId("clearNoteBtn")?.addEventListener("click", () => {
    const text = byId<HTMLTextAreaElement>("noteText");
    if (text) {
      text.value = "";
      text.setCustomValidity("");
    }
    text?.focus();
  });
  byId<HTMLTextAreaElement>("noteText")?.addEventListener("input", (e) => {
    const text = e.currentTarget as HTMLTextAreaElement;
    if (text.value.length > 500) {
      text.setCustomValidity("Notes are limited to 500 characters.");
    } else {
      text.setCustomValidity("");
    }
  });
  byId("noteForm")?.addEventListener("submit", (e) => {
    const text = byId<HTMLTextAreaElement>("noteText");
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
    closeImprovementPopover,
  );
  byId("cancelImprovementBtn")?.addEventListener(
    "click",
    closeImprovementPopover,
  );
  byId<HTMLTextAreaElement>("improvementNote")?.addEventListener(
    "input",
    (e) => {
      const el = e.currentTarget as HTMLTextAreaElement;
      el.setCustomValidity(
        el.value.length > 200
          ? "Improvement notes are limited to 200 characters."
          : "",
      );
    },
  );
  byId<HTMLTextAreaElement>("improvementDetail")?.addEventListener(
    "input",
    (e) => {
      const el = e.currentTarget as HTMLTextAreaElement;
      el.setCustomValidity(
        el.value.length > 1000
          ? "Improvement details are limited to 1000 characters."
          : "",
      );
    },
  );
  byId("improvementForm")?.addEventListener("submit", (e) => {
    const noteEl = byId<HTMLTextAreaElement>("improvementNote");
    const detailEl = byId<HTMLTextAreaElement>("improvementDetail");
    let invalid: HTMLTextAreaElement | null = null;
    if (noteEl) {
      if (noteEl.value.length > 200) {
        noteEl.setCustomValidity(
          "Improvement notes are limited to 200 characters.",
        );
        invalid = noteEl;
      } else {
        noteEl.setCustomValidity("");
      }
    }
    if (detailEl) {
      if (detailEl.value.length > 1000) {
        detailEl.setCustomValidity(
          "Improvement details are limited to 1000 characters.",
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
    const form = byId<HTMLFormElement>("loanForm");
    const id = form?.dataset.profileId;
    if (!id || form.dataset.mode !== "edit" || !canCreateProfile()) return;
    showProfileManagerError("");
    try {
      const data = await postProfileJson<CopyProfileData>(
        `/profiles/${id}/copy`,
      );
      upsertProfileOption(data.profile);
      setCanCreateProfile(Boolean(data.can_create_profile));
      renderProfileGutter();
      selectProfile(data.profile.id);
    } catch (err) {
      const message =
        err instanceof Error ? err.message : "Could not copy profile.";
      showProfileManagerError(message);
    }
  });
  byId("deleteProfileBtn")?.addEventListener("click", async () => {
    const form = byId<HTMLFormElement>("loanForm");
    const id = form?.dataset.profileId;
    if (!id || form.dataset.mode !== "edit") return;
    const opt = profileOptionById(id);
    if (!isOptionOwner(opt)) return;
    const name = opt?.dataset.name || "this profile";
    if (!confirm(`Delete profile “${name}”? This cannot be undone.`)) return;
    const select = byId<HTMLSelectElement>("profileSelect");
    const wasActive =
      select?.value === id || dashboard()?.dataset.profileId === id;
    showProfileManagerError("");
    try {
      const data = await postProfileJson<DeleteProfileData>(
        `/profiles/${id}/delete`,
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
      const message =
        err instanceof Error ? err.message : "Could not delete profile.";
      showProfileManagerError(message);
    }
  });
  byId("sharePanel")?.addEventListener("click", (e) => {
    const btn = asElement(e.target)?.closest<HTMLElement>("#copyShareLinkBtn");
    if (!btn) return;
    const input = byId<HTMLInputElement>("shareInviteUrl");
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
    toggleMenu("accountMenu", e.currentTarget as HTMLElement);
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

    const yearToggle = target.closest<HTMLElement>(".year-toggle");
    if (yearToggle) {
      e.preventDefault();
      const group = yearToggle.closest<HTMLElement>(".pay-year-group");
      if (!group) return;
      const next = !isYearGroupExpanded(group);
      setYearGroupExpanded(group, next);
      rememberYearExpanded(group.dataset.year, next);
      return;
    }
    const noteBtn = target.closest<HTMLElement>(".note-btn");
    if (noteBtn) {
      e.preventDefault();
      openNotePopover(noteBtn);
      return;
    }
    const improvementAddBtn = target.closest<HTMLElement>(
      ".improvement-add-btn",
    );
    if (improvementAddBtn) {
      e.preventDefault();
      openImprovementPopover(improvementAddBtn, "add");
      return;
    }
    const improvementEditBtn = target.closest<HTMLElement>(
      ".improvement-edit-btn",
    );
    if (improvementEditBtn) {
      e.preventDefault();
      openImprovementPopover(improvementEditBtn, "edit");
      return;
    }
    const grainBtn = target.closest<HTMLElement>(
      "#panel-chart .seg-toggle [data-grain]",
    );
    if (grainBtn) {
      e.preventDefault();
      setChartGrain(grainBtn.dataset.grain);
      return;
    }
    const tab = target.closest<HTMLElement>(".tab[data-tab]");
    if (tab) {
      e.preventDefault();
      activateTab(tab.dataset.tab);
    }
  });
  document.body.addEventListener("dblclick", (e) => {
    const target = asElement(e.target);
    if (!target) return;

    // Arrow/label button stays single-click; swallow its dblclick so two
    // clicks don't get followed by a third toggle from the row handler.
    if (target.closest(".year-toggle")) {
      e.preventDefault();
      return;
    }

    const yearSummary = target.closest("tr.year-summary");
    if (yearSummary) {
      e.preventDefault();
      const group = yearSummary.closest<HTMLElement>(".pay-year-group");
      if (!group) return;
      const next = !isYearGroupExpanded(group);
      setYearGroupExpanded(group, next);
      rememberYearExpanded(group.dataset.year, next);
      return;
    }

    const payYear = target.closest("summary.pay-year");
    if (payYear) {
      e.preventDefault();
      const group = payYear.closest<HTMLDetailsElement>(
        "details.pay-year-group",
      );
      if (!group) return;
      // Two summary clicks already cancelled out; flip once for the dblclick.
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
      panel
        ?.querySelectorAll<HTMLElement>(
          `tbody.pay-year-group[data-year="${year}"]`,
        )
        .forEach((other) => {
          setYearGroupExpanded(other, expanded);
        });
    },
    true,
  );
  document.body.addEventListener("change", (e) => {
    const t = e.target;
    if (!(t instanceof HTMLElement)) return;
    const form = t.closest<HTMLFormElement>("#extraForm");
    if (!form) return;
    if (t.matches('input[name="recast"], input[name="recurring"]')) {
      syncExtraFormOptions(form);
    }
  });
  const queryTab = new URLSearchParams(location.search).get("tab");
  // One-time migrate old #tab bookmarks to ?tab=.
  const legacyHashTab = location.hash.replace(/^#/, "");
  const dash = dashboard();
  // Prefer ?tab= over the server-rendered default so refresh keeps the active tab.
  const initialTab = normalizeTab(
    queryTab || legacyHashTab || dash?.dataset.tab || "calendar",
  );
  TAB_IDS.forEach((id) => {
    const panel = panelEl(id);
    if (panel) panel.dataset.stale = "false";
  });
  applyPaymentYearCollapse();
  bindPaymentsActionsPopover();
  activateTab(initialTab, { focus: false, syncUrl: true });
}

function installGlobalListeners(): void {
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
      const path =
        e.detail.pathInfo?.requestPath ||
        e.detail.elt?.getAttribute?.("hx-post") ||
        "";
      if (/\/improvements\/?$/.test(path)) {
        requestAnimationFrame(() => {
          window.scrollTo({
            top: document.documentElement.scrollHeight,
            behavior: "smooth",
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

function boot(): void {
  installGlobalListeners();
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", bindUi);
  } else {
    bindUi();
  }
}

boot();
