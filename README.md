# cosmos-oms

An [OpenC3](https://openc3.com) plugin that bridges OMS UCI messages into a cosmos deployment. The plugin consists of a Rust microservice binary (`oms_bridge`) that connects to an OMS ASB bus, receives UCI messages, and re-publishes them as cosmos telemetry packets over TCP. The `OMS` target defines the telemetry structure; the microservice and interface are wired up via `plugin.txt`.

## Quickstart
 1. `docker compose up -d`
 2. Go to the [OpenC3 Console](http://127.0.0.1:2900). You may need to wait a couple of minutes for the system to come up.
 3.

## Configuration

### Field mapping

By default the bridge auto-detects fields from every UCI message using XSD heuristics. To control exactly which fields appear as COSMOS telemetry items — and how they are typed — create `config/oms_field_mappings.toml`:

```toml
[[message]]
name = "SystemStatus"           # UCI element name (matches XSD element)

[[message.field]]
path = "MessageData/SystemId"   # slash-separated path from message root

[[message.field]]
path = "MessageData/SystemState"
cosmos_name = "SYS_STATE"       # optional: override the COSMOS item name

[[message.field]]
path = "MessageData/TimeUp"
type = "uint32"                 # optional: override auto-detected type
cosmos_name = "UPTIME_SECONDS"

[[message]]
name = "ServiceStatus"

[[message.field]]
path = "MessageData/ServiceId"

[[message.field]]
path = "MessageData/ServiceState"
```

Field paths use PascalCase XSD element names. The `type` override accepts: `uint8`, `uint16`, `uint32`, `uint64`, `int8`, `int16`, `int32`, `int64`, `float32`, `float64`, `bool`, `uuid`, `enum`, `timestamp_ms`, `string:N` (N bytes). Auto-detection resolves the XSD type directly — explicit overrides are only needed when the inferred type is wrong for your use case.

Messages not listed in the file fall back to heuristic detection (unchanged from the default behaviour).

Point to an alternate file at runtime:

```sh
OMS_FIELD_MAPPINGS_PATH=/path/to/my_mappings.toml
```

Or set the plugin variable `oms_field_mappings_path` when installing via the Admin panel.

### Environment variables

| Variable | Default | Description |
|---|---|---|
| `COSMOS_PROJECT_DIR` | `../cosmos-project` | Path to your cosmos-project checkout |
| `OMS_CONFIG_DIR` | `./config` | Directory containing `CALConfig.toml` |
| `OMS_BRIDGE_PORT` | `7779` | TCP port the bridge listens on (host-side, for debugging) |
| `OMS_FIELD_MAPPINGS_PATH` | `config/oms_field_mappings.toml` | Path to the field mapping config |

### Rebuilding after code changes

```sh
docker compose up --build openc3-operator
```

---

## Manual install (existing COSMOS deployment)

Use this path if you already have a running COSMOS deployment and want to install the plugin without Docker.

### Prerequisites

- A running [OpenC3 COSMOS](https://docs.openc3.com/docs/getting-started/installation) deployment
- An OMS system publishing UCI messages over DDS/ASB

### 1. Build the binary

```sh
cargo build --release --locked
```

### 2. Configure

Edit `CALConfig.toml` — at minimum set the DDS transport URI and a unique system ID:

```toml
[system]
id = "my-cosmos-oms-bridge"

[[transport]]
id = "oms-bus"
type = "dds"
uri = "udp://239.255.0.1:7400"
```

This file must be accessible inside the cosmos microservice container at runtime. Mount it into the container or place it on a shared volume.

### 3. Install the binary

The cosmos microservice container runs the Ruby launcher (`microservices/oms_bridge_microservice.rb`), which spawns the `oms_bridge` binary as a subprocess. Install the binary somewhere the microservice container can access it:

```sh
install -m 755 target/release/oms_bridge /path/on/cosmos-host/oms_bridge
```

### 4. Install the plugin in OpenC3

Upload the plugin via the **Admin** panel → **Plugins** → **Install**, selecting `plugin.txt` and the `targets/` and `microservices/` directories. Set the plugin variables when prompted:

| Variable | Description | Default |
|---|---|---|
| `oms_bridge_binary` | Full path to `oms_bridge` inside the microservice container | `oms_bridge` |
| `oms_calconfig_path` | Path to `CALConfig.toml` inside the microservice container | `CALConfig.toml` |
| `oms_field_mappings_path` | Path to field mapping TOML inside the microservice container | `/config/oms_field_mappings.toml` |
| `oms_bridge_port` | TCP port the bridge listens on | `7779` |

Alternatively, use the OpenC3 CLI:

```sh
openc3.sh cli load cosmos-oms-plugin.gem
```

### 5. Verify telemetry

Once the plugin is installed and the OMS source is active, open the OpenC3 **Telemetry Viewer** and select target `OMS`. The following packets should appear:

- `SYSTEM_STATUS` — overall system state
- `SERVICE_STATUS` — per-service state and uptime
- `SUBSYSTEM_STATUS` — per-subsystem operational state
