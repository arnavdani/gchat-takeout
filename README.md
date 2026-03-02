# GChat Takeout

Google Chat Takeout data is often an unorganized mess of JSON files and scattered media. **GChat Takeout** is a high-performance desktop browser that transforms these raw logs into a searchable, relational, and private experience. It solves the frustration of digging through directories by providing a familiar, modern chat interface for all your historical DMs and Spaces—completely offline.

## Usage

1.  Download the latest `.dmg` from the **GitHub Releases** page.
2.  Install by dragging the app to your `Applications` folder and launch it.
3.  Click **+ Import** in the sidebar.
4.  Select your extracted `Google Chat` directory from a Google Takeout export.
5.  Wait for the instant metadata import; your chat history will be ready to browse immediately.

## Tech Stack & Implementation

The application is built using **Tauri 2.0**, combining a high-performance **Rust** backend with a modern **React/TypeScript** frontend. It was designed with a strict **"privacy-first"** constraint: all message data remains on your local machine with zero remote server calls.

To achieve "super ridiculous fast search," the app pre-indexes the entire Takeout export into a local **SQLite** database. This relational model maps users, group memberships, and messages, allowing for instant filtering by participant, email, or message text. The implementation uses a high-speed Rust parser to walk the directory tree and ingest tens of thousands of messages in seconds.

### Key Features
- **Relational Data:** SQLite handles complex lookups, enabling the app to name DMs by the other participant and sort the sidebar by the most recent conversation.
- **Lazy Media Management:** To keep the initial import instant, the app parses metadata first. Files (images/videos) are copied into managed application storage lazily as you browse or via a low-priority background task.
- **Native UX:** Built specifically for macOS with support for native keyboard navigation (Arrow keys, `Cmd+F` for context-aware search), a draggable sidebar, and native macOS menu bar settings.
- **Attachments:** Full support for images (JPG, PNG, BMP, etc.) and video playback (MP4, MOV) rendered directly within the conversation stream.

The core logic resides in a custom Rust processing engine that handles the heavy lifting of data normalization and local file management, while the frontend provides a 0-latency interface for data that Google otherwise makes difficult to access.
