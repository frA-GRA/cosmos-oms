# cosmos-oms

An [OpenC3](https://openc3.com) plugin that bridges OMS UCI messages into a cosmos deployment. The plugin consists of a Rust microservice binary (`oms_bridge`) that connects to an OMS DDS/ASB bus, receives UCI messages, and re-publishes them as cosmos telemetry packets over TCP. The `OMS` target defines the telemetry structure; the microservice and interface are wired up via `plugin.txt`.

## Getting Started

### Prerequisites

- [Docker](https://docs.docker.com/get-docker/) with BuildKit enabled (Docker 23+ has it on by default)
- A running [OpenC3 COSMOS](https://docs.openc3.com/docs/getting-started/installation) deployment
- An OMS system publishing UCI messages over DDS/ASB

### 1. Build the image

```sh
docker build -t cosmos-oms .
```

This produces a minimal static binary image. The `oms_bridge` binary is compiled for `x86_64-unknown-linux-musl` and requires no shared libraries.

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
# adjust to your DDS endpoint
uri = "udp://239.255.0.1:7400"
```

This file must be accessible inside the cosmos microservice container at runtime. Mount it into the container or place it on a shared volume.

### 3. Extract the binary

The cosmos microservice container runs the Ruby launcher (`microservices/oms_bridge_microservice.rb`), which spawns the `oms_bridge` binary as a subprocess. Extract the binary from the image and place it somewhere the microservice container can access it:

```sh
docker create --name oms-extract cosmos-oms
docker cp oms-extract:/oms_bridge /path/on/cosmos-host/oms_bridge
docker rm oms-extract
```

Make the binary executable:

```sh
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
