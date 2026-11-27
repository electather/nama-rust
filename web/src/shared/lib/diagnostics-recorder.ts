// The OpenTelemetry SDK, in a chunk of its own that `./diagnostics` loads only when a Server
// exports Diagnostics (ADR 0054). What a span may carry: "Spans carry ids, not words" in `web/AGENTS.md`.
import { Code, ConnectError, type Interceptor } from "@connectrpc/connect";
import { type Attributes, SpanKind, SpanStatusCode } from "@opentelemetry/api";
import { OTLPTraceExporter } from "@opentelemetry/exporter-trace-otlp-proto";
import { resourceFromAttributes } from "@opentelemetry/resources";
import { BatchSpanProcessor, TracerProvider } from "@opentelemetry/sdk-trace";

/**
 * What records the web app's Diagnostics for the Server at `origin`, `""` for the page's own, and
 * posts them to it, with the `headers` it asks for at each post, such as its bearer token.
 */
export function recorderAt(origin: string, headers: () => Record<string, string>) {
  const provider = new TracerProvider({
    resource: resourceFromAttributes({ "service.name": "nama-web" }),
    // Each Server forwards them to its collector and the Hub, so the browser needs no collector's
    // address or key, and only the Hub's page needs CORS (ADR 0054, ADR 0105). Protobuf, since the
    // Hub takes no other encoding.
    spanProcessors: [
      new BatchSpanProcessor({
        exporter: new OTLPTraceExporter({
          url: `${origin}/diagnostics/v1/traces`,
          // oxlint-disable-next-line require-await -- the exporter takes its headers as a promise
          headers: async () => headers(),
        }),
      }),
    ],
  });
  const tracer = provider.getTracer("nama-web");

  /** Runs a ConnectRPC call in a span, sending its W3C `traceparent` so the Server's spans join it. */
  const traceCall: Interceptor = (next) => async (request) => {
    const { typeName } = request.service;
    const { name } = request.method;
    const span = tracer.startSpan(`${typeName}/${name}`, {
      kind: SpanKind.CLIENT,
      attributes: { "rpc.system": "connect_rpc", "rpc.service": typeName, "rpc.method": name },
    });
    const { traceId, spanId, traceFlags } = span.spanContext();
    request.header.set(
      "traceparent",
      `00-${traceId}-${spanId}-${traceFlags.toString(16).padStart(2, "0")}`,
    );
    try {
      return await next(request);
    } catch (error) {
      span.setStatus({ code: SpanStatusCode.ERROR });
      span.setAttribute("error.type", Code[ConnectError.from(error).code]);
      throw error;
    } finally {
      span.end();
    }
  };

  /** Records `name` as having taken from `startTime`, a `performance.now()`, until now. */
  function recordSpan(name: string, startTime: number, attributes: Attributes = {}) {
    tracer.startSpan(name, { startTime, attributes }).end();
  }

  /**
   * Records an error span `name`d, of `kind`, such as `TypeError`. Never its message: it may quote a
   * title.
   */
  function recordError(kind: string, name = "uncaught error") {
    const span = tracer.startSpan(name, { attributes: { "error.type": kind } });
    span.setStatus({ code: SpanStatusCode.ERROR });
    span.end();
  }

  return { traceCall, recordSpan, recordError };
}
