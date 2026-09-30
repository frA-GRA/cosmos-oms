# cosmos-oms

An [OpenC3](https://openc3.com) plugin that bridges OMS UCI messages into a cosmos deployment. The plugin consists of a Rust microservice binary (`oms_bridge`) that connects to an OMS DDS/ASB bus, receives UCI messages, and re-publishes them as cosmos telemetry packets over TCP. The `OMS` target defines the telemetry structure; the microservice and interface are wired up via `plugin.txt`.

## Docker (recommended)

Brings up a complete COSMOS stack with the OMS plugin pre-installed in a single `docker compose up`.

### Prerequisites

- [Docker](https://docs.docker.com/get-docker/) with Docker Compose ≥ 2.23 (Docker Desktop 4.25+)
- A clone of [cosmos-project](https://github.com/OpenC3/cosmos) on the same machine

### 1. One-time setup

**Linux / macOS:**

```sh
# Default assumes cosmos-project is at ../cosmos-project.
# Override with: export COSMOS_PROJECT_DIR=/path/to/cosmos-project
chmod +x setup.sh
./setup.sh
```

**Windows (PowerShell):**

```powershell
# Default assumes cosmos-project is at ..\cosmos-project.
# Override with: $env:COSMOS_PROJECT_DIR = "C:\path\to\cosmos-project"
.\setup.ps1
```

The script:
- Builds the `cosmos-oms-operator` image (Rust binary + plugin gem embedded)
- Installs the plugin gem into the cosmos-project `plugins/` directory for auto-install on startup
- Creates `config/CALConfig.toml` from the example if it does not exist

### 2. Configure

Edit `config/CALConfig.toml` — at minimum set the DDS transport URI and a unique system ID:

```toml
[system]
id = "my-cosmos-oms-bridge"

[[transport]]
id = "oms-bus"
type = "dds"
uri = "udp://239.255.0.1:7400"  # adjust to your DDS endpoint
```

### 3. Run

```sh
docker compose up -d
```

COSMOS is available at <http://localhost:2900>. On first start the init container installs all plugins (including `cosmos-oms`) automatically via COSMOS local mode. This takes about a minute.

To follow init progress:

```sh
docker compose logs -f openc3-cosmos-init
```

### 4. Verify telemetry

Open the COSMOS **Telemetry Viewer** and select target `OMS`. The following packets should appear once the OMS source is active:

- `SYSTEM_STATUS` — overall system state
- `SERVICE_STATUS` — per-service state and uptime
- `SUBSYSTEM_STATUS` — per-subsystem operational state

### Environment variables

| Variable | Default | Description |
|---|---|---|
| `COSMOS_PROJECT_DIR` | `../cosmos-project` | Path to your cosmos-project checkout |
| `OMS_CONFIG_DIR` | `./config` | Directory containing `CALConfig.toml` |
| `OMS_BRIDGE_PORT` | `7779` | TCP port the bridge listens on (host-side, for debugging) |

### Rebuilding after code changes

```sh
docker build -f Dockerfile.operator -t cosmos-oms-operator .
docker compose up -d openc3-operator
```

---

## Manual install (existing COSMOS deployment)

Use this path if you already have a running COSMOS deployment and want to install the plugin without Docker.

### Prerequisites

- [Docker](https://docs.docker.com/get-docker/) with BuildKit enabled (Docker 23+)
- A running [OpenC3 COSMOS](https://docs.openc3.com/docs/getting-started/installation) deployment
- An OMS system publishing UCI messages over DDS/ASB

### 1. Build the image

```sh
docker build -t cosmos-oms .
```

### 2. Configure

Copy the example CAL config and fill in your deployment values:

```sh
cp CALConfig.toml.example CALConfig.toml
```

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

### 3. Extract the binary

The cosmos microservice container runs the Ruby launcher (`microservices/oms_bridge_microservice.rb`), which spawns the `oms_bridge` binary as a subprocess. Extract the binary from the image and place it somewhere the microservice container can access it:

```sh
docker create --name oms-extract cosmos-oms
docker cp oms-extract:/oms_bridge /path/on/cosmos-host/oms_bridge
docker rm oms-extract
chmod +x /path/on/cosmos-host/oms_bridge
```

### 4. Install the plugin in OpenC3

Upload the plugin via the **Admin** panel → **Plugins** → **Install**, selecting `plugin.txt` and the `targets/` and `microservices/` directories. Set the plugin variables when prompted:

| Variable | Description | Default |
|---|---|---|
| `oms_bridge_binary` | Full path to `oms_bridge` inside the microservice container | `oms_bridge` |
| `oms_calconfig_path` | Path to `CALConfig.toml` inside the microservice container | `CALConfig.toml` |
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
