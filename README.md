# RoboScape Online (Rust Version)

# Crates
## roboscapesim-server
This crate is the server for the RoboScape simulation. It is responsible for managing simulations and the communication with the clients.

## roboscapesim-client
This crate is the client for the RoboScape simulation, including the NetsBlox extension and the WASM module.

## roboscapesim-common
This crate contains common code shared between the server and the client, along with the other crates.

## roboscapesim-api
This crate contains the "API server" for the RoboScape simulation. It handles load balancing and coordination between the simulation server and the clients.

## roboscapesim-client-common
This crate contains common code shared between the client and other client-like applications such as the preflight check.

## roboscapesim-preflight
This crate is a preflight check for the RoboScape simulation. It is used to check if the simulation is compatible with the current system. It verifies that you are able to connect to the API server, create a room, and receive the simulation state.
