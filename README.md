# Taxy

A TCP reverse proxy with a user-friendly WebUI.

[![Crates.io](https://img.shields.io/crates/v/taxy.svg)](https://crates.io/crates/taxy)
[![GitHub license](https://img.shields.io/github/license/picoHz/taxy.svg)](https://github.com/picoHz/taxy/blob/main/LICENSE)

## Overview

- Written in Rust for performance and safety
- Intuitive and user-friendly WebUI for easy configuration
- Live configuration updates without restarting the service

## Installation

To build the Taxy binary, ensure that you have the Rust toolchain installed.

Once you have successfully built and started taxy, you can access the admin panel at http://localhost:46492/.

### From crates.io

The package on crates.io contains the WebUI as a static asset, so you don't need to build it yourself.

Install "Taxy" using Cargo:

```bash
cargo install taxy
```

### From git

To build the Web UI, make sure you have Node.js installed on your system.

Clone the repository and install the package:

```bash
git clone https://github.com/picoHz/taxy
cd taxy/webui
npm install
npm run build
cd ..
cargo install --path .
```

## Development

To contribute or develop Taxy, follow these steps:

```bash
# Clone the repository
git clone https://github.com/picoHz/taxy

# Start the server
cd taxy
cargo run

# In a separate terminal, start Vite for the WebUI
cd webui
npm install
npm run dev