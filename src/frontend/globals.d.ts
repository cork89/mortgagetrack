/** Ambient globals loaded via <script> in templates/base.html */

interface HtmxAjaxOptions {
  target?: Element | string;
  swap?: string;
  headers?: Record<string, string>;
  values?: Record<string, unknown>;
}

interface HtmxApi {
  ajax(
    method: string,
    url: string,
    options?: HtmxAjaxOptions,
  ): Promise<void> | void;
  process(element: Element): void;
  trigger(element: Element, eventName: string): void;
}

interface AgChartInstance {
  update(options: unknown): void;
  destroy(): void;
}

interface AgChartsNamespace {
  AgCharts: {
    create(options: unknown): AgChartInstance;
  };
}

interface HtmxRequestConfig {
  triggeringEvent?: Event;
  target?: Element | string | null;
}

interface HtmxConfigRequestDetail {
  elt: Element;
  headers: Record<string, string>;
  parameters: Record<string, unknown>;
  verb?: string;
}

interface HtmxBeforeRequestDetail {
  elt: Element;
  requestConfig: HtmxRequestConfig;
}

interface HtmxAfterRequestDetail {
  elt: Element;
  successful: boolean;
  requestConfig: HtmxRequestConfig;
}

interface HtmxSwapDetail {
  target: HTMLElement;
  elt?: Element;
  pathInfo?: { requestPath?: string };
}

interface HtmxBeforeSwapDetail {
  target: HTMLElement | null;
}

interface MarkPanelsStaleDetail {
  keep?: import("./types").TabId;
  invalidateChart?: boolean;
}

declare const htmx: HtmxApi | undefined;
declare const agCharts: AgChartsNamespace | undefined;

interface DocumentEventMap {
  "htmx:configRequest": CustomEvent<HtmxConfigRequestDetail>;
  "htmx:beforeRequest": CustomEvent<HtmxBeforeRequestDetail>;
  "htmx:afterRequest": CustomEvent<HtmxAfterRequestDetail>;
  "htmx:beforeSwap": CustomEvent<HtmxBeforeSwapDetail>;
  "htmx:afterSwap": CustomEvent<HtmxSwapDetail>;
  "htmx:afterSettle": CustomEvent<HtmxSwapDetail>;
  markPanelsStale: CustomEvent<MarkPanelsStaleDetail>;
}

interface HTMLElementEventMap {
  "htmx:configRequest": CustomEvent<HtmxConfigRequestDetail>;
  "htmx:beforeRequest": CustomEvent<HtmxBeforeRequestDetail>;
  "htmx:afterRequest": CustomEvent<HtmxAfterRequestDetail>;
  "htmx:beforeSwap": CustomEvent<HtmxBeforeSwapDetail>;
  "htmx:afterSwap": CustomEvent<HtmxSwapDetail>;
  "htmx:afterSettle": CustomEvent<HtmxSwapDetail>;
  markPanelsStale: CustomEvent<MarkPanelsStaleDetail>;
}
