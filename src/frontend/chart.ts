import { byId, dashboard, money, panelEl } from "./core";
import type { ChartBucket, ChartGrain } from "./types";

interface BreakdownTooltipParams {
  datum?: ChartBucket;
  0?: { datum?: ChartBucket };
}

let breakdownChart: AgChartInstance | null = null;

function chartGrain(): ChartGrain {
  return dashboard()?.dataset.grain === "yearly" ? "yearly" : "monthly";
}

function cssVar(name: string): string {
  return getComputedStyle(document.documentElement)
    .getPropertyValue(name)
    .trim();
}

function parseBuckets(raw: string | undefined): ChartBucket[] {
  try {
    const data: unknown = JSON.parse(raw || "[]");
    if (!Array.isArray(data)) return [];
    return data.filter((row): row is ChartBucket => {
      if (!row || typeof row !== "object") return false;
      const r = row as Record<string, unknown>;
      return (
        typeof r.label === "string" &&
        typeof r.year === "number" &&
        typeof r.principal === "number" &&
        typeof r.interest === "number" &&
        typeof r.payment === "number"
      );
    });
  } catch {
    return [];
  }
}

export function destroyBreakdownChart(): void {
  if (!breakdownChart) return;
  try {
    breakdownChart.destroy();
  } catch {
    /* already gone with the DOM */
  }
  breakdownChart = null;
}

function breakdownTooltipHtml(params: BreakdownTooltipParams): string {
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

function yearAxisTickValues(data: ChartBucket[]): string[] {
  const firstLabelByYear = new Map<number, string>();
  for (const row of data) {
    if (row?.year == null || firstLabelByYear.has(row.year)) continue;
    firstLabelByYear.set(row.year, row.label);
  }
  const years = [...firstLabelByYear.keys()].sort((a, b) => a - b);
  const ticks: string[] = [];
  for (let i = 0; i < years.length; i += 2) {
    const year = years[i];
    if (year == null) continue;
    const label = firstLabelByYear.get(year);
    if (label != null) ticks.push(label);
  }
  return ticks;
}

function breakdownChartOptions(
  container: HTMLElement,
  data: ChartBucket[],
): Record<string, unknown> {
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
          formatter: ({ value }: { value: string }) => {
            const year = yearByLabel.get(value);
            if (year != null) return String(year);
            const match = String(value).match(/(\d{4})\s*$/);
            return match?.[1] ? match[1] : String(value);
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
          formatter: ({ value }: { value: number }) => money(value),
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
    tooltip: {
      mode: "single",
    },
  };
}

export function renderBreakdownChart(): void {
  const panel = panelEl("chart");
  const wrap = byId("chartWrap");
  const container = byId("breakdownChart");
  if (!panel?.classList.contains("active") || !wrap || !container) return;
  if (typeof agCharts === "undefined" || !agCharts.AgCharts) return;

  const grain = chartGrain();
  const data = parseBuckets(
    grain === "yearly"
      ? wrap.dataset.yearlyBuckets
      : wrap.dataset.monthlyBuckets,
  );
  const options = breakdownChartOptions(container, data);

  if (breakdownChart) {
    breakdownChart.update(options);
    return;
  }
  breakdownChart = agCharts.AgCharts.create(options);
}

export function setChartGrain(grain: string | undefined): void {
  const panel = panelEl("chart");
  if (!panel) return;
  const next: ChartGrain = grain === "yearly" ? "yearly" : "monthly";
  panel
    .querySelectorAll<HTMLElement>(".seg-toggle [data-grain]")
    .forEach((btn) => {
      const on = btn.dataset.grain === next;
      btn.classList.toggle("active", on);
      btn.setAttribute("aria-pressed", on ? "true" : "false");
    });
  panel.querySelectorAll<HTMLElement>("[data-grain-hint]").forEach((el) => {
    el.classList.toggle("hidden", el.dataset.grainHint !== next);
  });
  const dash = dashboard();
  if (dash) dash.dataset.grain = next;
  renderBreakdownChart();
}
