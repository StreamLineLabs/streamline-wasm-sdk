# Examples

## Prerequisites

- Modern browser, running Streamline server
- A running Streamline server (default: `localhost:9092`)

## Running

Start Streamline:

```bash
# Via Docker
STREAMLINE_FIXTURE_IMAGE=registry.example/streamline:test
docker run -p 9092:9092 -p 9094:9094 "$STREAMLINE_FIXTURE_IMAGE" --playground

# Or via Homebrew
streamline --playground
```

## Examples

### Quick Start (HTML)

Interactive browser-based demo with produce/consume UI:

```bash
open examples/quickstart.html
```

### Basic Usage (JavaScript)

Standalone script showing produce, consume, and admin operations:

```bash
# With a bundler project
npm install @streamlinelabs/streamline-wasm
node examples/basic-usage.js
```

Or include in an HTML page:

```html
<script type="module" src="examples/basic-usage.js"></script>
```

### Playground (HTML)

Full-featured interactive playground with topic management:

```bash
open examples/playground.html
```

## Configuration

Set `STREAMLINE_BOOTSTRAP_SERVERS` to connect to a non-local server:

```bash
export STREAMLINE_BOOTSTRAP_SERVERS=my-server:9092
Open examples/quickstart.html in a browser
```
