# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Tauri v2 desktop EPUB reader application with React + TypeScript frontend. The app loads EPUB files from a local directory, displays book covers in a library view, and provides access to book content and metadata through a custom URI protocol.

## Architecture

- **Frontend**: React 18 + TypeScript + Vite
  - Entry point: `src/main.tsx`
  - Main component: `src/App.tsx`
  - Routing: React Router v7 for navigation between views
  - Vite dev server runs on port 1420
  - Uses `@tauri-apps/api` to invoke Rust backend commands

- **Backend**: Rust (Tauri v2)
  - Entry point: `src-tauri/src/main.rs` (delegates to lib.rs)
  - Application logic: `src-tauri/src/lib.rs`
  - Build script: `src-tauri/build.rs`
  - Tauri commands are defined with `#[tauri::command]` macro (starting at `lib.rs:20`)
  - Commands are registered in the `invoke_handler` (see `lib.rs:263`)
  - Script injection function `inject_link_handler_script()` handles external link interception (`lib.rs:92-168`)

- **Communication**: Frontend calls backend via `invoke()` function from `@tauri-apps/api/core`
  - Example: `await invoke("greet", { name })` calls the Rust `greet` function

## Navigation Flow

The application uses React Router for URL-based navigation:

1. **Library View (`/`)**
   - Displays grid of all books with covers
   - Click on any book navigates to `/book/:bookKey`

2. **Reading View (`/book/:bookKey`)**
   - Extracts bookKey from URL params
   - Fetches Table of Contents via `get_book_toc` command
   - Displays ToC sidebar and content viewer
   - Loads first chapter by default
   - Clicking ToC items updates content: `epub://${bookKey}/${content}`
   - Back button returns to `/` (Library View)

### Content Loading Flow in BookReader

```typescript
// 1. Extract bookKey from URL
const { bookKey } = useParams<{ bookKey: string }>();

// 2. Fetch ToC on mount
const tocData = await invoke<TocItem[]>("get_book_toc", { bookKey });

// 3. Build epub:// URI for first chapter
const firstContent = tocData[0].content;
setCurrentContent(`epub://${bookKey}/${firstContent}`);

// 4. When user clicks ToC item
function handleTocItemClick(content: string) {
  const fullUri = `epub://${bookKey}/${content}`;
  setCurrentContent(fullUri);
}

// 5. IframeViewer loads the URI via custom protocol
<IframeViewer uri={currentContent} />
```

## EPUB Reader Features

### Book Library
- **Location**: EPUB files are loaded from `/Users/richardwooding/books` (configurable in `lib.rs:89`)
- **State Management**: Books are stored in `LibraryState` - an `Arc<Mutex<HashMap<String, EpubDoc>>>` for thread-safe access
- **Error Handling**: Corrupted EPUB files are logged and skipped during loading

### Custom URI Protocol: `epub://`
The application registers a custom `epub://` protocol handler to serve EPUB resources:
- Format: `epub://<book-filename>/<resource-path>`
- Example: `epub://frankenstein.epub/OEBPS/cover.jpg`
- Handles: Images, HTML/XHTML pages, CSS, and other EPUB resources
- Returns appropriate MIME types and content
- **Script Injection**: Automatically injects JavaScript into HTML/XHTML content to handle link clicks (see `lib.rs:92-168`)

### External Link Handling
The application intercepts link clicks within EPUB content and opens external links in the system's default browser:

**How it works:**
1. **Backend (Rust)**: `inject_link_handler_script()` function in `lib.rs` injects JavaScript into all HTML/XHTML content served via the `epub://` protocol
2. **Injected Script**: Intercepts all clicks on `<a>` tags using event delegation
3. **Link Classification**:
   - **Relative links** (e.g., `chapter2.html`) → Navigate within iframe
   - **epub:// protocol links** → Navigate within iframe
   - **All other protocols** (http://, https://, mailto:, tel:, ftp:, etc.) → Open externally
4. **Communication**: External links send a postMessage to the parent window
5. **Frontend (React)**: IframeViewer component listens for messages and uses Tauri's opener plugin to open links in system browser

**Link Behavior:**
| Link Type | Example | Behavior |
|-----------|---------|----------|
| Relative | `chapter2.html` | Navigate within iframe |
| Anchor | `#section-2` | Scroll within iframe |
| EPUB protocol | `epub://book.epub/page.html` | Navigate within iframe |
| HTTP/HTTPS | `https://example.com` | Open in system browser |
| Email | `mailto:test@example.com` | Open in email client |
| Telephone | `tel:+1234567890` | Open in dialer |
| FTP | `ftp://example.com` | Open in system browser/FTP client |
| Other protocols | `steam://...` | Open with system handler |

### Available Tauri Commands

#### `all_book_covers() -> Vec<(String, String, String)>`
Returns list of all books with cover information:
- Tuple format: `(book_key, book_title, cover_uri)`
- Automatically extracts cover images from EPUB metadata
- Falls back to searching resources for cover images if needed

#### `get_book_toc(book_key: String) -> Result<Vec<TocItem>, String>`
Returns the Table of Contents for a specific EPUB:
```rust
struct TocItem {
    label: String,          // Chapter title
    content: String,        // Path to content file
    play_order: usize,      // Reading order
    children: Vec<TocItem>, // Nested chapters
}
```

### React Components

#### `BookLibrary` (`src/components/BookLibrary.tsx`)
- Main library view displaying all EPUB covers in a responsive grid
- Calls `all_book_covers` command on mount
- Displays book titles and filenames
- Click handler navigates to BookReader view using React Router
- Supports dark mode
- Grid adapts to window size (250px min per card, 180px on mobile)

#### `BookReader` (`src/components/BookReader.tsx`)
- Full reading interface with table of contents and content viewer
- Uses URL params to get bookKey via `useParams<{ bookKey: string }>()`
- Layout: Fixed header + flexbox content area (ToC left, content right)
- Fetches ToC on mount via `invoke("get_book_toc", { bookKey })`
- Automatically loads first chapter as default content
- Header includes:
  - Back button (← Library) to return to BookLibrary
  - Book title (derived from bookKey, .epub extension removed)
- Handles loading and error states
- Supports dark mode

#### `TableOfContents` (`src/components/TableOfContents.tsx`)
- Recursive component for displaying nested chapter hierarchy
- Props: `toc: TocItem[]`, `onItemClick: (content: string) => void`
- Fixed width sidebar (300px) with vertical scrolling
- All chapters expanded by default
- Nested items indented by 20px per level
- Click handler triggers content loading in BookReader
- Dark mode support

#### `IframeViewer` (`src/components/IframeViewer.tsx`)
- Reusable component for rendering EPUB content in iframes
- Props: `uri`, `title`, `width`, `height`, `style`, `className`
- Used for HTML/XHTML content rendering in BookReader
- Security: Uses sandbox attribute with limited permissions (`allow-same-origin allow-scripts allow-forms`)

**Implementation Details:**
- **Single-mechanism src control**: Uses useEffect to imperatively set `iframeRef.current.src = uri` (more reliable than React's src prop for custom protocols)
- **No JSX src prop**: The iframe element has no `src` attribute in JSX to avoid double-loading
- **External link handling**: Listens for postMessage events from injected script in iframe content
- **Message validation**: Validates message type, URL, and origin before processing
- **Tauri integration**: Uses `openUrl()` from `@tauri-apps/plugin-opener` to open external links in system browser

**Why this approach:**
React's `src` prop doesn't reliably trigger iframe reloads with custom protocols like `epub://`. The useEffect directly invokes the browser's iframe src setter, which ensures consistent reload behavior without double-loading or flickering.

### TypeScript Types (`src/types/book.ts`)

```typescript
export interface TocItem {
  label: string;          // Chapter title
  content: string;        // Path to content file in EPUB
  play_order: number;     // Reading order
  children: TocItem[];    // Nested chapters
}
```

This interface matches the Rust `TocItem` struct returned by `get_book_toc`.

### Styling
- Dark mode support via `@media (prefers-color-scheme: dark)`
- Responsive grid layout for book covers
- Minimal whitespace design with content starting at top
- Book covers scale to fit using `object-fit: contain`
- Fixed 300px sidebar for Table of Contents
- Flexbox layout for BookReader (sidebar + content area)

## Development Commands

### Running the application in development mode
```bash
pnpm tauri dev
```
This starts both the Vite dev server (frontend) and Tauri application (backend) concurrently.

### Building for production
```bash
pnpm build  # Builds frontend (TypeScript compilation + Vite build)
pnpm tauri build  # Builds the complete desktop application
```

### Frontend-only development
```bash
pnpm dev  # Runs just the Vite dev server
```

### Type checking
```bash
npx tsc --noEmit  # Type check without emitting files
```

### Rust development
```bash
cd src-tauri
cargo build  # Build Rust backend only
cargo check  # Quick check without full compilation
cargo test   # Run Rust tests
```

## Adding New Tauri Commands

1. Define the command in `src-tauri/src/lib.rs` with `#[tauri::command]`
2. Add it to the `invoke_handler` macro (currently at `lib.rs:263`):
   ```rust
   .invoke_handler(tauri::generate_handler![greet, all_book_covers, get_book_toc, new_command])
   ```
3. Call it from frontend:
   ```typescript
   import { invoke } from "@tauri-apps/api/core";
   const result = await invoke("new_command", { args });
   ```

## Working with EPUBs

### Adding Books
Place `.epub` files in `/Users/richardwooding/books` and restart the application.

### Reading Books
1. Launch the application - it opens to the library view showing all books
2. Click any book card to open it in the reader
3. Use the Table of Contents on the left to navigate chapters
4. Click "← Library" button to return to the library view

### Accessing EPUB Resources
Use the `epub://` protocol to reference any resource within an EPUB:
```typescript
// Cover image
<img src="epub://my-book.epub/OEBPS/images/cover.jpg" />

// Chapter content
<IframeViewer uri="epub://my-book.epub/OEBPS/chapter1.xhtml" />
```

### EPUB Metadata Access
Book metadata is extracted during loading:
- Title: Available via `book.mdata("title")`
- Cover: Automatically extracted and provided as image URI
- ToC: Available via `get_book_toc` command

## Configuration Files

- `src-tauri/tauri.conf.json`: Tauri application configuration (window size, build commands, bundle settings)
- `src-tauri/Cargo.toml`: Rust dependencies and package configuration
- `package.json`: Node.js dependencies and npm scripts
- `vite.config.ts`: Vite bundler configuration (port 1420, HMR settings)
- `tsconfig.json`: TypeScript compiler options (strict mode enabled)

## Project Structure

```
epub-reader/
├── src/                          # Frontend source
│   ├── components/               # React components
│   │   ├── BookLibrary.tsx       # Main library grid view
│   │   ├── BookLibrary.css       # Library styling (includes dark mode)
│   │   ├── BookReader.tsx        # Book reading interface (ToC + content)
│   │   ├── BookReader.css        # Reader layout styles
│   │   ├── TableOfContents.tsx   # Recursive ToC component
│   │   ├── TableOfContents.css   # ToC styling
│   │   ├── IframeViewer.tsx      # Reusable iframe component
│   │   └── index.ts              # Component exports
│   ├── types/
│   │   └── book.ts               # TypeScript interfaces (TocItem)
│   ├── App.tsx                   # Main app component with routing
│   ├── App.css                   # Global app styles
│   └── main.tsx                  # React entry point
│
├── src-tauri/                    # Rust backend
│   ├── src/
│   │   ├── lib.rs                # Main application logic & commands
│   │   └── main.rs               # Entry point (delegates to lib.rs)
│   ├── Cargo.toml                # Rust dependencies (epub, serde, etc.)
│   └── tauri.conf.json           # Tauri configuration
│
├── dist/                         # Frontend build output
├── src-tauri/target/             # Rust build output
└── /Users/richardwooding/books/  # EPUB files location
```

## Key Dependencies

### Rust
- `epub = "2.0"` - EPUB parsing and metadata extraction
- `tauri = "2.9"` - Desktop app framework
- `tauri-plugin-opener = "2"` - Opens URLs in system browser/applications
- `serde = { version = "1", features = ["derive"] }` - Serialization
- `http = "1.2"` - HTTP types for custom protocol

### Frontend
- `react = "^18.3"` - UI framework
- `react-router-dom = "^7.1"` - URL-based routing and navigation
- `@tauri-apps/api = "^2.3"` - Tauri frontend bindings
- `@tauri-apps/plugin-opener = "^2"` - Opens URLs in system browser/applications
- `vite = "^6.0"` - Build tool and dev server
