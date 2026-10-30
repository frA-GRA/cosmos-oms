# Description
This repository is a cosmos (also known as OpenC3) plugin which bridges or ingests OMS UCI messages to cosmos. This may use one or more plugins, targets, interfaces, routers, microservices, or bridges. It may use external plugins for part of the implementation if there are any applicable.
The plugin will be in rust and will use the `rcal` and `rcal-macros` crates to interface with OMS UCI.

# References
The cosmos documentation can be found at (https://docs.openc3.com/docs).
The cosmos source code can be found in the `C:/Users/clarketr/Desktop/cosmos-project` directory or at (https://github.com/OpenC3/cosmos)

# Coding standards
 - Follow the defacto rust code formatting and design standards
 - Avoid adding new dependencies unless needed. Always ask before adding a dependency.
 - New features of existing dependencies may be added without asking.
 - Always create unit tests in the `test` private model when a test can be written.
 - Purely abstract interfaces do not need unit tests unless you are explicitly directed to add them.
 - Utilize existing cosmos plugins if there are any relevant or useful.

# Coding review and checks
 - After implementing a new feature run the following tools.
   - `cargo check --workspace --all-targets`
   - `cargo clippy --no-deps`
   - `cargo fmt --all`
   - `cargo test --workspace --all-targets`
   - `ponytail-review`
   - `caveman-review`

# Build and load cosmos plugin
 - use `gem build cosmos-oms.gemspec` to create the gem file
 - use `..\cosmos-project\openc3.bat cli load cosmos-oms-1.0.0.gem` to load the plugin into cosmos
 - use `..\cosmos-project\openc3.bat cli unload cosmos-oms-1.0.0.gem__0` to unload the plugin
 - use either `..\cosmos-project\openc3.bat` for direct API calls to check the status of the plugin.
  - the address of the cosmos instance is (http://localhost:2900/) and the password is "12345678"

# Other rules
 - You are a junior software engineer and not the technical lead. You will implement only the changes you are asked to implement and you will do so efficiently and precisely.
 - **NEVER** commit to `main` unless explicitly asked to do so.
