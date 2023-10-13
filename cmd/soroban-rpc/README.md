# Soroban-RPC

Soroban-RPC allows you to communicate directly with Soroban via a JSON RPC interface.

For example, you can build an application and have it send a transaction, get ledger and event data or simulate transactions.

Alternatively, you can use one of Soroban's client SDKs such as the js-soroban-client, which will need to communicate with an RPC instance to access the network.

## Dependencies
  - [Git](https://git-scm.com/downloads)
  - [Go](https://golang.org/doc/install)
  - [Rust](https://www.rust-lang.org/tools/install)
  - [Cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html)

## Building Stellar-Core
Soroban-RPC requires an instance of stellar-core binary on the same host. This is referred to as the `Captive Core`. 
Since, we are building RPC from source, we recommend considering two approaches to get the stellar-core binary:
- If saving time is top priority and your development machine is on a linux debian OS, then consider installing the 
testnet release candidates from the [testing repository.](https://apt.stellar.org/pool/unstable/s/stellar-core/)
- The recommended option is to compile the core source directly on your machine:
    - Clone the stellar-core repo:
        ```bash
        git clone https://github.com/stellar/stellar-core.git
        cd stellar-core
        ```
    - Fetch the tags and checkout the testnet release tag:
        ```bash
        git fetch --tags
        git checkout tags/v20.0.0-rc.2.1 -b soroban-testnet-release
        ```
    - Follow the build steps listed in [INSTALL.md](https://github.com/stellar/stellar-core/blob/master/INSTALL.md) file for the instructions on building the local binary

## Building Soroban-RPC
- Similar to stellar-core, we will clone the soroban-tools repo and checkout the testnet release tag:
```bash
git clone https://github.com/stellar/soroban-tools.git
cd soroban-tools
git fetch --tags
git checkout tags/v20.0.0-rc4 -b soroban-testnet-release
```
- Install the required go packages:
```bash
make install
```
- Build soroban-rpc target:
```bash
make build-soroban-rpc
```
This will generate a `soroban-rpc` binary in the working directory.

## Configuration
- Stellar-Core requires a configuration file to run. Here is a sample configuration file for testnet:
    ```toml
    DATABASE = "sqlite3://stellar.db"
    ENABLE_SOROBAN_DIAGNOSTIC_EVENTS = true
    EXPERIMENTAL_BUCKETLIST_DB = true
    EXPERIMENTAL_BUCKETLIST_DB_INDEX_PAGE_SIZE_EXPONENT = 12
    FAILURE_SAFETY = -1
    HTTP_PORT = 11626
    LOG_FILE_PATH = ""
    NETWORK_PASSPHRASE = "Test SDF Network ; September 2015"
    UNSAFE_QUORUM = true
    
    [[HOME_DOMAINS]]
    HOME_DOMAIN = "testnet.stellar.org"
    QUALITY = "HIGH"
    
    [[VALIDATORS]]
    ADDRESS = "core-testnet1.stellar.org"
    HISTORY = "curl -sf http://history.stellar.org/prd/core-testnet/core_testnet_001/{0} -o {1}"
    HOME_DOMAIN = "testnet.stellar.org"
    NAME = "sdftest1"
    PUBLIC_KEY = "GDKXE2OZMJIPOSLNA6N6F2BVCI3O777I2OOC4BV7VOYUEHYX7RTRYA7Y"
    
    [[VALIDATORS]]
    ADDRESS = "core-testnet2.stellar.org"
    HISTORY = "curl -sf http://history.stellar.org/prd/core-testnet/core_testnet_002/{0} -o {1}"
    HOME_DOMAIN = "testnet.stellar.org"
    NAME = "sdftest2"
    PUBLIC_KEY = "GCUCJTIYXSOXKBSNFGNFWW5MUQ54HKRPGJUTQFJ5RQXZXNOLNXYDHRAP"
    
    [[VALIDATORS]]
    ADDRESS = "core-testnet3.stellar.org"
    HISTORY = "curl -sf http://history.stellar.org/prd/core-testnet/core_testnet_003/{0} -o {1}"
    HOME_DOMAIN = "testnet.stellar.org"
    NAME = "sdftest3"
    PUBLIC_KEY = "GC2V2EFSXN6SQTWVYA5EPJPBWWIMSD2XQNKUOHGEKB535AQE2I6IXV2Z"
    ```
- For production, we recommend running Soroban RPC with a TOML configuration file rather than CLI flags. There is a new subcommand `gen-config-file` which takes all the same arguments as the root command (or no arguments at all), and outputs the resulting config toml file to stdout. 
    ```bash
    ./soroban-rpc gen-config-file
    ```
- Paste the output to a file and save it as `.toml` file in any directory. Make sure to update the config values to testnet specific ones.

## Running RPC Server
If everything is set up correctly, then you can run the RPC server with the following command:
```bash
./soroban-rpc --config-path <PATH_TO_THE_RPC_CONFIG_FILE>
```