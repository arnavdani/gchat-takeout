# Use a base image with Node and Rust (1.85+ required for Edition 2024)
FROM rust:1.85-slim-bookworm

# Install Node.js, npm, and system dependencies for Tauri/SQLite
RUN apt-get update && apt-get install -y 
    curl 
    libwebkit2gtk-4.1-dev 
    build-essential 
    curl 
    wget 
    file 
    libxdo-dev 
    libssl-dev 
    libayatana-appindicator3-dev 
    librsvg2-dev 
    pkg-config 
    sqlite3 
    libsqlite3-dev

# Install Node.js 20.x
RUN curl -fsSL https://deb.nodesource.com/setup_20.x | bash - && 
    apt-get install -y nodejs

WORKDIR /app

# Copy package files first for caching
COPY package*.json ./
RUN npm install

# Copy the rest of the app
COPY . .

# Default command
CMD ["npm", "run", "tauri", "dev"]
