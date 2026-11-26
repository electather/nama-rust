import { create } from "@bufbuild/protobuf";
import { createRouterTransport } from "@connectrpc/connect";
import { createConnectQueryKey } from "@connectrpc/connect-query";
import { QueryClient } from "@tanstack/react-query";
import { describe, expect, test } from "vite-plus/test";

import { applyEvent } from "@/app/data/apply-event";
import {
  type Event,
  EventSchema,
  InboxChangedSchema,
  RequestCancelledSchema,
  RequestCreatedSchema,
  RequestStateChangedSchema,
  SubscribeResponseSchema,
} from "@/gen/nama/v1/event_pb";
import { CountUnreadInboxNotificationsResponseSchema, InboxService } from "@/gen/nama/v1/inbox_pb";
import { RequestSchema } from "@/gen/nama/v1/request_pb";

describe("applying an Event to the inbox", () => {
  const transport = createRouterTransport(() => {});
  const unreadKey = createConnectQueryKey({
    schema: InboxService.method.countUnreadInboxNotifications,
    input: {},
    transport,
    cardinality: "finite",
  });

  /** Whether `event` leaves the cached unread count needing a refetch. */
  function recounts(event: Event["event"]) {
    const queryClient = new QueryClient();
    queryClient.setQueryData(unreadKey, create(CountUnreadInboxNotificationsResponseSchema));
    const response = create(SubscribeResponseSchema, {
      message: { case: "event", value: create(EventSchema, { event }) },
    });
    applyEvent(response, { queryClient, transport });
    return queryClient.getQueryState(unreadKey)?.isInvalidated;
  }

  test("an inbox change recounts it", () => {
    const value = create(InboxChangedSchema, { userId: 1n });
    expect(recounts({ case: "inboxChanged", value })).toBe(true);
  });

  test("a Request event leaves it, as its notification may not be stored yet", () => {
    const request = create(RequestSchema, { id: 8n });
    const created = create(RequestCreatedSchema, { request });
    const changed = create(RequestStateChangedSchema, { request });
    const cancelled = create(RequestCancelledSchema, { request });
    expect(recounts({ case: "requestCreated", value: created })).toBe(false);
    expect(recounts({ case: "requestStateChanged", value: changed })).toBe(false);
    expect(recounts({ case: "requestCancelled", value: cancelled })).toBe(false);
  });
});
