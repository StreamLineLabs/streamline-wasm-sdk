# Streamline WASM SDK — Browser Demo

Interactive browser demo for the Streamline WASM SDK. Lets you connect to a Streamline server, manage topics, produce messages, and consume them — all from the browser using WebAssembly.

## Quick Start

### 1. Build the WASM package

```bash
# From the repository root
wasm-pack build --target web
```

This generates the `pkg/` directory with the compiled `.wasm` binary and JavaScript bindings.

### 2. Serve the demo locally

A local HTTP server is needed (WASM modules cannot be loaded via `file://`).

```bash
# From the repository root
python3 -m http.server 8080
```

Then open [http://localhost:8080/demo/](http://localhost:8080/demo/) in your browser.

### 3. Connect to Streamline

Enter your Streamline server's WebSocket URL (default: `ws://localhost:9094/ws`) and click **Connect**.

## Demo Mode

If the WASM package is not built or no server is available, the demo automatically runs in **Demo Mode**:

- Simulated connection with mock WebSocket state
- Pre-populated topic list (create/delete topics locally)
- Producer sends messages that echo in the consumer log
- Consumer receives simulated streaming events every 2–3 seconds
- Full UI interaction without any backend

Toggle between Demo Mode and Live Mode using the button in the Connection card.

## What the Demo Shows

| Section    | Functionality                                              |
|------------|------------------------------------------------------------|
| Connection | Connect/disconnect to a Streamline server via WebSocket    |
| Topics     | List, create, and delete topics via `TopicAdmin` API       |
| Producer   | Send messages with optional key and headers to any topic   |
| Consumer   | Subscribe to a topic and see messages in a real-time log   |
| Status Bar | Live connection state, mode indicator, and message counter |

## Screenshot

<!-- TODO: Add screenshot -->
![Demo Screenshot](screenshot.png)

## Architecture

```
Browser (demo/index.html)
  │
  ├─ import('../pkg/streamline_wasm_sdk.js')  ← WASM module
  │    └─ StreamlineClient, Producer, Consumer, TopicAdmin
  │
  └─ WebSocket ──▶ Streamline Server (ws://host:9094/ws)
```

## Requirements

- Modern browser with WebAssembly support (Chrome, Firefox, Safari, Edge)
- [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/) for building
- Python 3 (or any static HTTP server) for serving locally
- A running [Streamline](https://github.com/streamlinelabs/streamline) server for live mode
