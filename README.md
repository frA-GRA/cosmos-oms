# cosmos-oms

OpenC3 COSMOS plugin that bridges OMS UCI messages into COSMOS telemetry over DDS.

The Rust binary (`oms_bridge`) subscribes to OMS UCI topics via `rcal`, encodes each
status message into a COSMOS-compatible binary packet, and forwards it to COSMOS over
a local TCP socket.  Telemetry definitions are generated at build time from the UCI XSD
so the COSMOS packet layout always matches the wire types.

---

## Repository layout

```
cosmos-oms/          ← this repo
../cosmos-project/   ← OpenC3/COSMOS (sibling directory, or set COSMOS_PROJECT_DIR)
../rcal/             ← rcal/rcal-macros crates (required for the Docker build)
```

Clone this repo next to `cosmos-project`:

```sh
cd <workspace>
git clone https://github.com/OpenC3/cosmos cosmos-project   # if not already present
git clone <this-repo-url> cosmos-oms
```

---

## Quick-start with Docker Compose

This spins up a full COSMOS stack plus the OMS bridge in one command.

### Prerequisites

- Docker Desktop 4.25+ (Compose v2.23+)
- `cosmos-project` checked out at `../cosmos-project` (or set `COSMOS_PROJECT_DIR`)
- `rcal` crate sources at `../rcal` (required to build `adsb-sim` / `asb-dump`)

### Start everything

```sh
docker compose up -d
```

COSMOS will be available at <http://localhost:2900> (password: `12345678`).

To point at a cosmos-project in a different location:

```sh
COSMOS_PROJECT_DIR=/path/to/cosmos-project docker compose up -d
```

or export it in your shell:

```sh
export COSMOS_PROJECT_DIR=/path/to/cosmos-project
docker compose up -d
```

### Configuration

| File | Purpose |
|---|---|
| `config/CALConfig.toml` | rcal transport and subscribed UCI topics |
| `config/oms_field_mappings.toml` | Which XSD fields to expose as COSMOS items |

Mount overrides are picked up automatically — see `docker-compose.yml` (`OMS_CONFIG_DIR`).

### Stop / tear down

```sh
docker compose down
```

---

## Installing into an existing COSMOS instance

Use this path when COSMOS is already running (started via `openc3.bat start` /
`openc3.sh start`) and you only want to add the OMS plugin.

### 1. Build the gem

Requires Rust (stable), Ruby, and the `rcal` crate at `../rcal/rcal`.

```sh
# Generate telemetry definitions and the oms_bridge binary
cargo build --release

# Package the gem (picks up targets/OMS/cmd_tlm/oms_generated.txt from the build)
gem build cosmos-oms.gemspec
```

This produces `cosmos-oms-1.0.0.gem`.

### 2. Load the plugin

```sh
# Windows
..\cosmos-project\openc3.bat cli load cosmos-oms-1.0.0.gem

# Linux / macOS
../cosmos-project/openc3.sh cli load cosmos-oms-1.0.0.gem
```

COSMOS will prompt for plugin variables (ports, config paths, etc.).
The defaults in `plugin_instance.json` work out of the box for a local install.

### 3. Verify

```sh
# Windows
..\cosmos-project\openc3.bat

# Linux / macOS
../cosmos-project/openc3.sh
```

Then check <http://localhost:2900> — the `OMS` target should appear in CmdTlmServer.

### Unload

```sh
# Windows
..\cosmos-project\openc3.bat cli unload cosmos-oms-1.0.0.gem__0

# Linux / macOS
../cosmos-project/openc3.sh cli unload cosmos-oms-1.0.0.gem__0
```

---

## Connecting `oms_bridge` to DDS

`oms_bridge` uses rcal's DDS transport.  The DDS domain participant must be able to
discover OMS peers.

### From another Docker container (e.g. `adsb-sim`)

The `docker-compose.yml` attaches all services to the `cosmos-project_default` network.
Any container on that network can reach the DDS domain.  No extra configuration is
needed — rcal defaults to DDS domain 0 via the `uri = "0"` setting in `CALConfig.toml`.

If your OMS source container is on a different Compose project, attach it to the same
network:

```yaml
# in your other compose file
services:
  your-oms-source:
    networks:
      - cosmos-project_default

networks:
  cosmos-project_default:
    external: true
    name: cosmos-project_default
```

### From the host machine

Docker's default bridge network does **not** route multicast, which DDS relies on for
peer discovery.  To bridge DDS between host and container use one of:

**Option A — host network mode (Linux only)**

```yaml
services:
  openc3-operator:
    network_mode: host
```

**Option B — set a unicast peer address**

Set `OMS_ZMQ_PEER` (or `RCAL_INITIAL_PEERS`) to the host's IP in `plugin.txt` /
`plugin_instance.json`, or pass it as an environment variable:

```toml
# config/CALConfig.toml — add under [[transport]]
initial_peers = ["<host-ip>"]
```

**Option C — run `oms_bridge` directly on the host**

Build with the native target instead of musl and run the binary outside Docker.
It will share the host DDS domain with other processes automatically:

```sh
cargo build --release
RCAL_CALCONFIG_PATH=config/CALConfig.toml ./target/release/oms_bridge
```

Then point the COSMOS interface at `127.0.0.1:7780`.

---

## Adding telemetry translations

Telemetry definitions are driven by two config files that are read at **build time**.
After editing either file, rebuild (`cargo build --release` or `docker compose build`)
to regenerate `targets/OMS/cmd_tlm/oms_generated.txt` and re-package the gem.

### 1. Subscribe to a UCI topic — `config/CALConfig.toml`

Add a `[[service.topic]]` entry under the relevant `[[service]]` block:

```toml
[[service]]
id = "oms_ingest"

[[service.topic]]
id = "SystemStatus"

[[service.topic]]
id = "YourNewTopic"    # must match the XSD element name exactly
```

Topics listed here are the only ones the bridge subscribes to.  The build script
cross-references this list against the UCI XSD to generate packet definitions.

### 2. Select fields — `config/oms_field_mappings.toml`

Each `[[message]]` block maps one UCI message type to COSMOS items.

```toml
[[message]]
name = "YourNewTopic"          # XSD element name

  [[message.field]]
  path = "MessageData/SomeId"  # slash-separated XSD path from the message root

  [[message.field]]
  path = "MessageData/SomeGroup/SomeValue"
  cosmos_name = "MY_ITEM"      # optional — defaults to UPPER_SNAKE of the leaf name
  type = "float64"             # optional — overrides auto-detection
```

**Path rules:**
- Use PascalCase names exactly as they appear in the XSD (e.g. `SystemId`, not `system_id`).
- Slash-separate nested elements: `MessageData/Kinematics/Position/…`.

**Type auto-detection** (no `type` key needed in most cases):

| XSD type | COSMOS encoding |
|---|---|
| `*ID_Type` complexType | `STRING 288` (UUID) |
| enum `simpleType` | `UINT 8` + `STATE` entries |
| `xs:duration`-based | `UINT 32` (seconds) |
| `xs:dateTime` / `xs:time` | `INT 64` (ms since Unix epoch) |
| `xs:boolean` | `UINT 8` (`STATE FALSE/TRUE`) |
| `xs:float` / `xs:double` | `FLOAT 32/64` |
| `xs:unsignedInt` etc. | `UINT 8/16/32/64` |
| `xs:string` / `xs:token` | `STRING 512` (64 bytes, zero-padded) |

**Explicit `type` values:** `uint8`, `uint16`, `uint32`, `uint64`, `int8`, `int16`,
`int32`, `int64`, `float32`, `float64`, `bool`, `uuid`, `enum`, `timestamp_ms`,
`string:<N>` (N bytes).

Messages not listed in `oms_field_mappings.toml` fall back to heuristic detection:
UUID fields, `*State*` enum fields, and `TimeUp` duration are captured automatically.

### 3. Rebuild and reinstall

```sh
cargo build --release
gem build cosmos-oms.gemspec
..\cosmos-project\openc3.bat cli unload cosmos-oms-1.0.0.gem__0
..\cosmos-project\openc3.bat cli load cosmos-oms-1.0.0.gem
```

Or with Docker Compose:

```sh
docker compose build openc3-operator
docker compose up -d
```
