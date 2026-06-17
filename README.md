# Remote Build Utility

A desktop application and build backend that compiles local C/C++ source code inside a Docker container.

## Architecture

- **Tauri App (`app/`)**: Provides the desktop interface (HTML/CSS/JS) to browse folders and trigger compilation.
- **Actix Server (`server/`)**: Manages the local HTTP endpoints (`/api/sync`, `/api/compile`, `/api/output`).
- **Docker (`docker/`)**: Contains the GCC compilation container image and build scripts.

## Setup & Running

For step-by-step setup instructions, see the [SETUP.md](SETUP.md) guide.

### Quick Start

1. **Build Container**:
   ```bash
   docker build -t build-container docker/
   ```

2. **Start Backend**:
   ```bash
   cd server
   cargo run --release
   ```

3. **Start Tauri App**:
   ```bash
   cd app
   npm install
   npm run tauri dev
   ```
