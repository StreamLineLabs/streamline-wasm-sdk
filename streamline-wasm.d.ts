/**
 * TypeScript type definitions for @streamlinelabs/streamline-wasm
 *
 * Browser-native WebAssembly SDK for the Streamline streaming platform.
 * These types supplement the wasm-bindgen generated types in pkg/.
 *
 * @packageDocumentation
 */

/**
 * Error codes returned by Streamline operations.
 */
export enum ErrorCode {
  NotConnected = "NotConnected",
  ConnectionFailed = "ConnectionFailed",
  SerializationError = "SerializationError",
  TopicNotFound = "TopicNotFound",
  AuthenticationFailed = "AuthenticationFailed",
  Timeout = "Timeout",
  ProduceError = "ProduceError",
  AdminError = "AdminError",
  QueryError = "QueryError",
  SchemaRegistryError = "SchemaRegistryError",
  Unknown = "Unknown",
}

/**
 * Error type returned by Streamline operations.
 * Includes error code, human-readable message, retryable flag, and contextual hint.
 */
export class StreamlineError extends Error {
  /** The error code classifying this error. */
  readonly code: ErrorCode;
  /** Whether this error is retryable (e.g., connection issues, timeouts). */
  readonly retryable: boolean;
  /** Returns a contextual hint for resolving this error. */
  hint(): string;

  free(): void;
}

/**
 * Initialize the WASM module. Must be called before using any SDK classes.
 *
 * @example
 * ```typescript
 * import init, { StreamlineClient } from '@streamlinelabs/streamline-wasm';
 *
 * await init();
 * const client = new StreamlineClient('ws://localhost:9094/ws');
 * ```
 */
export default function init(
  input?: RequestInfo | URL | WebAssembly.Module | BufferSource
): Promise<void>;

/**
 * WebSocket connection states.
 */
export enum ConnectionState {
  Disconnected = 0,
  Connecting = 1,
  Connected = 2,
  Reconnecting = 3,
}

/**
 * Callback invoked when a message is received from a subscribed topic.
 *
 * @param message - JSON string containing the message payload
 */
export type MessageCallback = (message: string) => void;

/**
 * Callback invoked when the connection state changes.
 *
 * @param state - The new connection state as a string ("Connected", "Disconnected", etc.)
 */
export type StateChangeCallback = (state: string) => void;

/**
 * High-level Streamline client for browser environments.
 *
 * Wraps a WebSocket connection and exposes produce, subscribe, and admin helpers.
 *
 * @example
 * ```typescript
 * import init, { StreamlineClient } from '@streamlinelabs/streamline-wasm';
 *
 * await init();
 * const client = new StreamlineClient('ws://localhost:9094/ws');
 * client.connect();
 *
 * // Produce a message
 * client.produce('my-topic', 'Hello from browser!');
 *
 * // Subscribe to a topic
 * client.subscribe('my-topic', (msg) => {
 *   console.log('Received:', msg);
 * });
 *
 * // Cleanup
 * client.disconnect();
 * ```
 */
export class StreamlineClient {
  /**
   * Create a new client targeting the given WebSocket URL.
   *
   * @param url - WebSocket URL (e.g., `ws://localhost:9094/ws`)
   */
  constructor(url: string);

  /**
   * Open the WebSocket connection.
   *
   * @throws If the WebSocket connection cannot be established
   */
  connect(): void;

  /**
   * Close the WebSocket connection.
   */
  disconnect(): void;

  /**
   * Returns `true` when the WebSocket is open and connected.
   */
  is_connected(): boolean;

  /**
   * Produce a message to the given topic.
   *
   * @param topic - Target topic name
   * @param value - Message value (string payload)
   * @throws If the WebSocket is not connected
   */
  produce(topic: string, value: string): void;

  /**
   * Produce a keyed message to the given topic.
   *
   * @param topic - Target topic name
   * @param key - Optional message key for partitioning
   * @param value - Message value (string payload)
   * @throws If the WebSocket is not connected
   */
  produce_with_key(topic: string, key: string | undefined, value: string): void;

  /**
   * Subscribe to messages on a topic.
   *
   * @param topic - Topic name to subscribe to
   * @param callback - Function invoked for every incoming message
   * @throws If the WebSocket is not connected
   */
  subscribe(topic: string, callback: MessageCallback): void;

  /**
   * Unsubscribe from a topic.
   *
   * @param topic - Topic name to unsubscribe from
   * @throws If the WebSocket is not connected
   */
  unsubscribe(topic: string): void;

  /**
   * Request topic creation via admin message.
   *
   * @param name - Name of the topic to create
   * @param partitions - Optional number of partitions (server default if omitted)
   * @throws If the WebSocket is not connected
   */
  create_topic(name: string, partitions?: number): void;

  /**
   * Request topic deletion via admin message.
   *
   * @param name - Name of the topic to delete
   * @throws If the WebSocket is not connected
   */
  delete_topic(name: string): void;

  /**
   * Request the list of topics via admin message.
   *
   * @throws If the WebSocket is not connected
   */
  list_topics(): void;

  /**
   * Register a callback for connection state changes.
   *
   * @param callback - Function invoked when the connection state changes
   */
  on_state_change(callback: StateChangeCallback): void;
}

/**
 * Convenience producer handle.
 *
 * Optionally bound to a default topic for simplified sending.
 *
 * @example
 * ```typescript
 * const producer = new Producer('ws://localhost:9094/ws', 'my-topic');
 * producer.connect();
 * producer.send('hello');           // uses default topic
 * producer.send('hello', 'other'); // override topic
 * producer.disconnect();
 * ```
 */
export class Producer {
  /**
   * Create a producer, optionally bound to a default topic.
   *
   * @param url - WebSocket URL
   * @param default_topic - Optional default topic for send operations
   */
  constructor(url: string, default_topic?: string);

  /**
   * Open the underlying WebSocket connection.
   *
   * @throws If the WebSocket connection cannot be established
   */
  connect(): void;

  /**
   * Send a message. Uses the default topic if none is specified.
   *
   * @param value - Message value
   * @param topic - Optional topic override
   * @throws If the WebSocket is not connected or no topic is available
   */
  send(value: string, topic?: string): void;

  /**
   * Send a keyed message.
   *
   * @param key - Message key for partitioning
   * @param value - Message value
   * @param topic - Optional topic override
   * @throws If the WebSocket is not connected or no topic is available
   */
  send_keyed(key: string, value: string, topic?: string): void;

  /**
   * Disconnect the producer.
   */
  disconnect(): void;
}

/**
 * Convenience consumer handle bound to a single topic.
 *
 * @example
 * ```typescript
 * const consumer = new Consumer('ws://localhost:9094/ws', 'my-topic');
 * consumer.start((msg) => console.log('Got:', msg));
 * // ... later
 * consumer.stop();
 * ```
 */
export class Consumer {
  /**
   * Create a consumer for the given topic.
   *
   * @param url - WebSocket URL
   * @param topic - Topic to consume from
   */
  constructor(url: string, topic: string);

  /**
   * Connect and subscribe in one step.
   *
   * @param callback - Function invoked for each received message
   * @throws If the WebSocket connection cannot be established
   */
  start(callback: MessageCallback): void;

  /**
   * Stop consuming and disconnect.
   */
  stop(): void;
}

/**
 * Topic administration helper.
 *
 * @example
 * ```typescript
 * const admin = new TopicAdmin('ws://localhost:9094/ws');
 * admin.connect();
 * admin.on_response((resp) => console.log(JSON.parse(resp)));
 * admin.create_topic('new-topic', 6);
 * admin.list_topics();
 * admin.disconnect();
 * ```
 */
export class TopicAdmin {
  /**
   * Create a topic admin client.
   *
   * @param url - WebSocket URL
   */
  constructor(url: string);

  /**
   * Open the WebSocket connection.
   *
   * @throws If the WebSocket connection cannot be established
   */
  connect(): void;

  /**
   * Create a topic with the given number of partitions.
   *
   * @param name - Topic name
   * @param partitions - Optional number of partitions
   * @throws If the WebSocket is not connected
   */
  create_topic(name: string, partitions?: number): void;

  /**
   * Delete a topic.
   *
   * @param name - Topic name to delete
   * @throws If the WebSocket is not connected
   */
  delete_topic(name: string): void;

  /**
   * Request the list of topics.
   *
   * @throws If the WebSocket is not connected
   */
  list_topics(): void;

  /**
   * Register a callback that receives admin responses.
   *
   * @param callback - Function invoked with JSON response strings
   */
  on_response(callback: MessageCallback): void;

  /**
   * Disconnect the admin client.
   */
  disconnect(): void;
}

/**
 * A lightweight span handle for browser-based telemetry.
 *
 * Holds information needed to close a performance measurement span.
 */
export class TelemetrySpan {
  /** The span label (e.g., "orders produce"). */
  readonly label: string;

  /** The topic associated with this span. */
  readonly topic: string;

  /** The operation type ("produce", "consume", or "process"). */
  readonly operation: string;
}

/**
 * Browser-native telemetry for Streamline operations.
 *
 * Uses `console.time`/`console.timeEnd` for console-visible timing and
 * `Performance.mark`/`Performance.measure` for DevTools integration.
 *
 * @example
 * ```typescript
 * const telemetry = new Telemetry();
 *
 * const span = telemetry.start_produce('orders');
 * client.produce('orders', 'message data');
 * telemetry.end_span(span);
 *
 * // Disable telemetry
 * telemetry.enabled = false;
 * ```
 */
export class Telemetry {
  /**
   * Create a new Telemetry instance (enabled by default).
   */
  constructor();

  /**
   * Create a disabled telemetry instance (all operations are no-ops).
   */
  static disabled(): Telemetry;

  /** Whether telemetry collection is enabled. */
  enabled: boolean;

  /**
   * Start timing a produce operation.
   *
   * @param topic - The topic being produced to
   * @returns A span handle to pass to `end_span()`
   */
  start_produce(topic: string): TelemetrySpan;

  /**
   * Start timing a consume operation.
   *
   * @param topic - The topic being consumed from
   * @returns A span handle to pass to `end_span()`
   */
  start_consume(topic: string): TelemetrySpan;

  /**
   * Start timing a record processing operation.
   *
   * @param topic - The topic being processed
   * @returns A span handle to pass to `end_span()`
   */
  start_process(topic: string): TelemetrySpan;

  /**
   * End a span, recording the performance measurement.
   *
   * @param span - The span handle from a `start_*` method
   */
  end_span(span: TelemetrySpan): void;

  /**
   * End a span and record an error.
   *
   * @param span - The span handle from a `start_*` method
   * @param error - Error message to record
   */
  end_span_with_error(span: TelemetrySpan, error: string): void;
}

/**
 * Generate a W3C traceparent header value.
 *
 * Creates a new random trace ID and span ID for distributed tracing.
 *
 * @returns A traceparent string in the format `00-{trace-id}-{span-id}-01`
 *
 * @example
 * ```typescript
 * const traceparent = generate_traceparent();
 * // "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"
 * ```
 */
export function generate_traceparent(): string;

/**
 * Parse a traceparent header into its components.
 *
 * @param traceparent - A W3C traceparent header string
 * @returns An object with `version`, `trace_id`, `span_id`, and `trace_flags` fields,
 *          or `null` if the header is invalid
 *
 * @example
 * ```typescript
 * const parsed = parse_traceparent('00-abc123...-def456...-01');
 * if (parsed) {
 *   console.log(parsed.trace_id, parsed.span_id);
 * }
 * ```
 */
export function parse_traceparent(traceparent: string): {
  version: string;
  trace_id: string;
  span_id: string;
  trace_flags: string;
} | null;

