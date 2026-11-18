// The OpenTelemetry SDK, in a chunk of its own that `./diagnostics` loads only when the Server
// exports Diagnostics (ADR 0054). What a span may carry: "Spans carry ids, not words" in `web/AGENTS.md`.
import { Code, ConnectError, type Interceptor } from "@connectrpc/connect";
import { type Attributes, SpanKind, SpanStatusCode } from "@opentelemetry/api";
import { OTLPTraceExporter } from "@opentelemetry/exporter-trace-otlp-http";
import { resourceFromAttributes } from "@opentelemetry/resources";
import { BatchSpanProcessor, TracerProvider } from "@opentelemetry/sdk-trace";

const provider = new TracerProvider({
  resource: resourceFromAttributes({ "service.name": "nama-web" }),
  // The Server forwards them to its collector, so the browser needs no CORS, address or key.
  spanProcessors: [
    new BatchSpanProcessor({ exporter: new OTLPTraceExporter({ url: "/diagnostics/v1/traces" }) }),
  ],
});
const tracer = provider.getTracer("nama-web");

/** Runs a ConnectRPC call in a span, sending its W3C `traceparent` so the Server's spans join it. */
export const traceCall: Interceptor = (next) => async (request) => {
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
export function recordSpan(name: string, startTime: number, attributes: Attributes = {}) {
  tracer.startSpan(name, { startTime, attributes }).end();
}

/** Records an uncaught error of `kind`, such as `TypeError`: its message may quote a title. */
export function recordError(kind: string) {
  const span = tracer.startSpan("uncaught error", { attributes: { "error.type": kind } });
  span.setStatus({ code: SpanStatusCode.ERROR });
  span.end();
}
