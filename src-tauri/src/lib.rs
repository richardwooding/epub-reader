use http::response::Builder as ResponseBuilder;
use epub::doc::EpubDoc;
use serde::Serialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::sync::{Arc, Mutex};

struct LibraryState(Arc<Mutex<HashMap<String, EpubDoc<BufReader<File>>>>>);

#[derive(Serialize, Clone)]
struct TocItem {
    label: String,
    content: String,
    play_order: usize,
    children: Vec<TocItem>,
}

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn all_book_covers(state: tauri::State<LibraryState>) -> Vec<(String, String, String)> {
    state.0.lock().unwrap().iter().filter_map(|book_entry | {
        let book_key = book_entry.0.clone();
        let book_title = book_entry.1.mdata("title").unwrap_or(book_key.replace(".epub", ""));

        // Try to get the cover image directly
        if let Ok(cover_id) = book_entry.1.get_cover_id() {
            // Get the actual cover resource (could be image or HTML)
            if let Some(cover_resource) = book_entry.1.resources.get(&cover_id) {
                let cover_path = cover_resource.0.to_str().unwrap();
                let mime_type = &cover_resource.1;

                // If it's already an image, use it directly
                if mime_type.starts_with("image/") {
                    let cover_uri = format!("epub://{}/{}", book_key, cover_path);
                    return Some((book_key, book_title, cover_uri));
                }

                // If it's HTML/XHTML, try to find the actual image in resources
                // Look for common cover image patterns
                for (res_path, (path_buf, res_mime)) in book_entry.1.resources.iter() {
                    if res_mime.starts_with("image/") &&
                       (res_path.contains("cover") || res_path.contains("Cover")) {
                        let image_path = path_buf.to_str().unwrap();
                        let cover_uri = format!("epub://{}/{}", book_key, image_path);
                        return Some((book_key, book_title, cover_uri));
                    }
                }

                // Fallback: use the HTML cover page
                let cover_uri = format!("epub://{}/{}", book_key, cover_path);
                return Some((book_key, book_title, cover_uri));
            }
        }

        // If no cover found, skip this book
        None
    }).collect()
}

#[tauri::command]
fn get_book_title(book_key: String, state: tauri::State<LibraryState>) -> Result<String, String> {
    let books = state.0.lock().unwrap();

    if let Some(book) = books.get(&book_key) {
        let title = book.mdata("title").unwrap_or(book_key.replace(".epub", ""));
        Ok(title)
    } else {
        Err(format!("Book not found: {}", book_key))
    }
}

#[tauri::command]
fn get_book_toc(book_key: String, state: tauri::State<LibraryState>) -> Result<Vec<TocItem>, String> {
    let mut books = state.0.lock().unwrap();

    if let Some(book) = books.get_mut(&book_key) {
        // Get the table of contents from the EPUB
        let toc = &book.toc;

        // Convert NavPoint to TocItem recursively
        fn convert_nav_points(nav_points: &[epub::doc::NavPoint]) -> Vec<TocItem> {
            nav_points.iter().map(|nav_point| {
                TocItem {
                    label: nav_point.label.clone(),
                    content: nav_point.content.to_str().unwrap_or("").to_string(),
                    play_order: nav_point.play_order,
                    children: convert_nav_points(&nav_point.children),
                }
            }).collect()
        }

        Ok(convert_nav_points(toc))
    } else {
        Err(format!("Book not found: {}", book_key))
    }
}

#[tauri::command]
fn get_spine(book_key: String, state: tauri::State<LibraryState>) -> Result<Vec<String>, String> {
    let books = state.0.lock().unwrap();

    if let Some(book) = books.get(&book_key) {
        // spine is Vec<String> of resource IDs
        // Convert to content paths using resources map
        let spine_paths: Vec<String> = book.spine.iter()
            .filter_map(|id| {
                book.resources.get(id).map(|(path, _)| {
                    path.to_str().unwrap_or("").to_string()
                })
            })
            .collect();

        Ok(spine_paths)
    } else {
        Err(format!("Book not found: {}", book_key))
    }
}

#[tauri::command]
fn get_current_spine_index(
    book_key: String,
    content_path: String,
    state: tauri::State<LibraryState>
) -> Result<Option<usize>, String> {
    let books = state.0.lock().unwrap();

    if let Some(book) = books.get(&book_key) {
        // Find index in spine where resource path matches content_path
        let index = book.spine.iter().position(|id| {
            book.resources.get(id)
                .map(|(path, _)| path.to_str().unwrap_or("") == content_path)
                .unwrap_or(false)
        });

        Ok(index)
    } else {
        Err(format!("Book not found: {}", book_key))
    }
}

#[tauri::command]
fn get_spine_item(
    book_key: String,
    index: usize,
    state: tauri::State<LibraryState>
) -> Result<Option<String>, String> {
    let books = state.0.lock().unwrap();

    if let Some(book) = books.get(&book_key) {
        if let Some(resource_id) = book.spine.get(index) {
            if let Some((path, _)) = book.resources.get(resource_id) {
                return Ok(Some(path.to_str().unwrap_or("").to_string()));
            }
        }
        Ok(None)
    } else {
        Err(format!("Book not found: {}", book_key))
    }
}

fn inject_link_handler_script(html_content: Vec<u8>) -> Vec<u8> {
    // Convert bytes to string
    let html_str = match String::from_utf8(html_content.clone()) {
        Ok(s) => s,
        Err(_) => {
            eprintln!("Failed to parse HTML as UTF-8, serving without injection");
            return html_content;
        }
    };

    // Default CSS for EPUB content with dark mode support
    let default_css = r#"<style>
/* Default styling for EPUB content - applied before EPUB's own CSS */
:root {
    color-scheme: light dark;
}

html, body {
    background-color: #ffffff;
    color: #1a1a1a;

    /* Responsive margins for comfortable reading */
    padding-left: max(16px, min(5vw, 80px));
    padding-right: max(16px, min(5vw, 80px));
    padding-top: max(16px, min(3vh, 48px));
    padding-bottom: max(32px, min(5vh, 64px));

    /* Optimal line width for readability */
    max-width: 50rem;
    margin-left: auto;
    margin-right: auto;

    /* Improve text rendering */
    line-height: 1.6;
}

a {
    color: #0066cc;
}

a:visited {
    color: #551a8b;
}

a:hover {
    color: #003d7a;
}

@media (prefers-color-scheme: dark) {
    html, body {
        background-color: #1e1e1e;
        color: #e4e4e4;

        /* Responsive margins for comfortable reading */
        padding-left: max(16px, min(5vw, 80px));
        padding-right: max(16px, min(5vw, 80px));
        padding-top: max(16px, min(3vh, 48px));
        padding-bottom: max(32px, min(5vh, 64px));

        /* Optimal line width for readability */
        max-width: 50rem;
        margin-left: auto;
        margin-right: auto;

        /* Improve text rendering */
        line-height: 1.6;
    }

    a {
        color: #58a6ff;
    }

    a:visited {
        color: #d2a8ff;
    }

    a:hover {
        color: #79b8ff;
    }
}

/* ============================================
   PAGINATION: CSS Multi-Column Layout
   ============================================ */

/* Base styles for html/body to ensure full height */
html {
    height: 100%;
    margin: 0;
    padding: 0;
    overflow: hidden;
}

body {
    height: 100%;
    margin: 0;
    padding: 0;
    overflow: hidden;
}

body.paginated {
    max-width: none !important;
    column-width: 100%;        /* Changed from 100vw - uses iframe width */
    column-gap: 0;
    column-fill: auto;
    height: 100%;              /* Changed from 100vh - uses iframe height */
    width: 100%;               /* Changed from 100vw - uses iframe width */
    overflow-x: hidden;        /* Hide horizontal scrollbar */
    overflow-y: hidden;        /* Hide vertical scrollbar */
    padding: 0 !important;
    margin: 0 !important;
    scroll-behavior: smooth;
}

body.paginated > * {
    padding-left: max(16px, min(5vw, 80px));
    padding-right: max(16px, min(5vw, 80px));
    padding-top: max(16px, min(3vh, 48px));
    padding-bottom: max(32px, min(5vh, 64px));
}

/* Prevent images and tables from breaking across columns */
body.paginated img,
body.paginated table {
    -webkit-column-break-inside: avoid;
    break-inside: avoid;
    max-width: 100%;
}

/* Dark mode pagination */
@media (prefers-color-scheme: dark) {
    body.paginated {
        background-color: #1e1e1e;
        color: #e4e4e4;
    }
}
</style>"#;

    // JavaScript to inject - includes pagination and link handling
    let script = r#"<script>
//<![CDATA[
(function() {
    'use strict';

    // ==========================================
    // PAGINATION STATE
    // ==========================================
    let paginationEnabled = false;
    let currentPage = 0;
    let totalPages = 0;
    let pageWidth = 0;

    // ==========================================
    // INITIALIZATION
    // ==========================================
    function initializePagination() {
        // Enable pagination by default
        enablePagination();

        // Listen for messages from parent
        window.addEventListener('message', handleParentMessage);

        // Recalculate on resize
        window.addEventListener('resize', debounce(calculatePages, 250));

        // Initial calculation after content loads
        if (document.readyState === 'loading') {
            document.addEventListener('DOMContentLoaded', function() {
                setTimeout(calculatePages, 100);
            });
        } else {
            setTimeout(calculatePages, 100);
        }
    }

    // ==========================================
    // PAGINATION CONTROL
    // ==========================================
    function enablePagination() {
        if (paginationEnabled) return;

        paginationEnabled = true;
        document.body.classList.add('paginated');

        setTimeout(function() {
            calculatePages();
            sendPaginationUpdate();
        }, 100);
    }

    function disablePagination() {
        if (!paginationEnabled) return;

        paginationEnabled = false;
        document.body.classList.remove('paginated');
        document.body.scrollLeft = 0;
    }

    // ==========================================
    // PAGE CALCULATION
    // ==========================================
    function calculatePages() {
        if (!paginationEnabled) return;

        // Use documentElement dimensions for accurate iframe measurements
        pageWidth = document.documentElement.clientWidth;
        const pageHeight = document.documentElement.clientHeight;

        // Set explicit column width and body height
        document.body.style.columnWidth = pageWidth + 'px';
        document.body.style.height = pageHeight + 'px';

        // Use requestAnimationFrame to wait for layout to complete
        requestAnimationFrame(function() {
            // Calculate total scroll width after layout
            const scrollWidth = document.body.scrollWidth;

            // Calculate total pages (at least 1)
            totalPages = Math.max(1, Math.ceil(scrollWidth / pageWidth));

            // Ensure current page is within bounds
            currentPage = Math.min(currentPage, totalPages - 1);

            // Navigate to current page
            navigateToPage(currentPage, false);

            // Notify parent
            sendPaginationUpdate();
        });
    }

    // ==========================================
    // NAVIGATION
    // ==========================================
    function navigateToPage(pageNumber, animated) {
        if (!paginationEnabled) return;

        // Default animated to true
        if (animated === undefined) animated = true;

        // Clamp page number
        pageNumber = Math.max(0, Math.min(pageNumber, totalPages - 1));

        // Update current page
        currentPage = pageNumber;

        // Calculate scroll position
        const scrollLeft = pageNumber * pageWidth;

        // Scroll to page
        if (animated) {
            document.body.scrollTo({
                left: scrollLeft,
                behavior: 'smooth'
            });
        } else {
            document.body.scrollLeft = scrollLeft;
        }

        // Notify parent
        sendPaginationUpdate();
    }

    function nextPage() {
        navigateToPage(currentPage + 1);
    }

    function previousPage() {
        navigateToPage(currentPage - 1);
    }

    // ==========================================
    // EVENT HANDLERS
    // ==========================================
    function handleKeyDown(event) {
        if (!paginationEnabled) return;

        switch (event.key) {
            case 'ArrowLeft':
                event.preventDefault();
                previousPage();
                break;
            case 'ArrowRight':
                event.preventDefault();
                nextPage();
                break;
            case 'PageUp':
                event.preventDefault();
                previousPage();
                break;
            case 'PageDown':
                event.preventDefault();
                nextPage();
                break;
            case 'Home':
                event.preventDefault();
                navigateToPage(0);
                break;
            case 'End':
                event.preventDefault();
                navigateToPage(totalPages - 1);
                break;
        }
    }

    function handleClick(event) {
        if (!paginationEnabled) return;

        // Check if click is on a link - if so, let link handler deal with it
        if (event.target.closest('a')) return;

        // Get click position relative to viewport
        const clickX = event.clientX;
        const viewportWidth = window.innerWidth;

        // Define click zones (20% left, 20% right, 60% middle)
        const leftZone = viewportWidth * 0.2;
        const rightZone = viewportWidth * 0.8;

        if (clickX < leftZone) {
            event.preventDefault();
            previousPage();
        } else if (clickX > rightZone) {
            event.preventDefault();
            nextPage();
        }
        // Middle zone does nothing (reserved for text selection, etc.)
    }

    let touchStartX = 0;
    let touchStartY = 0;

    function handleTouchStart(event) {
        if (!paginationEnabled) return;

        touchStartX = event.touches[0].clientX;
        touchStartY = event.touches[0].clientY;
    }

    function handleTouchEnd(event) {
        if (!paginationEnabled) return;

        const touchEndX = event.changedTouches[0].clientX;
        const touchEndY = event.changedTouches[0].clientY;

        const deltaX = touchEndX - touchStartX;
        const deltaY = touchEndY - touchStartY;

        // Only register horizontal swipes (more horizontal than vertical)
        if (Math.abs(deltaX) > Math.abs(deltaY) && Math.abs(deltaX) > 50) {
            event.preventDefault();

            if (deltaX > 0) {
                // Swipe right -> previous page
                previousPage();
            } else {
                // Swipe left -> next page
                nextPage();
            }
        }
    }

    // ==========================================
    // PARENT COMMUNICATION
    // ==========================================
    function handleParentMessage(event) {
        if (!event.data || typeof event.data !== 'object') return;

        const message = event.data;

        switch (message.type) {
            case 'pagination-next':
                nextPage();
                break;
            case 'pagination-previous':
                previousPage();
                break;
            case 'pagination-goto':
                if (typeof message.page === 'number') {
                    navigateToPage(message.page);
                }
                break;
            case 'pagination-enable':
                enablePagination();
                break;
            case 'pagination-disable':
                disablePagination();
                break;
        }
    }

    function sendPaginationUpdate() {
        if (window.parent && window.parent !== window) {
            window.parent.postMessage({
                type: 'pagination-update',
                currentPage: currentPage,
                totalPages: totalPages,
                enabled: paginationEnabled
            }, '*');
        }
    }

    // ==========================================
    // UTILITIES
    // ==========================================
    function debounce(func, wait) {
        let timeout;
        return function() {
            const context = this;
            const args = arguments;
            clearTimeout(timeout);
            timeout = setTimeout(function() {
                func.apply(context, args);
            }, wait);
        };
    }

    // ==========================================
    // LINK HANDLER (existing functionality)
    // ==========================================
    function handleLinkClick(event) {
        const target = event.target.closest('a');
        if (!target || !target.href) return;

        const href = target.href;

        if (isExternalLink(href)) {
            event.preventDefault();
            event.stopPropagation();

            if (window.parent && window.parent !== window) {
                window.parent.postMessage({
                    type: 'epub-external-link',
                    url: href
                }, '*');
            }
        }
    }

    function isExternalLink(href) {
        try {
            const url = new URL(href, window.location.href);
            const protocol = url.protocol.toLowerCase();

            if (protocol === 'epub:') return false;

            return true;
        } catch (e) {
            return false;
        }
    }

    // ==========================================
    // ATTACH EVENT LISTENERS
    // ==========================================
    document.addEventListener('click', handleLinkClick, true);
    document.addEventListener('keydown', handleKeyDown);
    document.addEventListener('click', handleClick);
    document.addEventListener('touchstart', handleTouchStart);
    document.addEventListener('touchend', handleTouchEnd);

    // Initialize pagination
    initializePagination();
})();
//]]>
</script>"#;

    // Find </head> or <body> tag to inject before
    let injection_point = if let Some(head_close) = html_str.find("</head>") {
        head_close
    } else if let Some(body_open) = html_str.find("<body") {
        // Find the end of the <body> tag
        html_str[body_open..].find('>').map(|pos| body_open + pos + 1)
            .unwrap_or(0)
    } else {
        // No ideal location found, inject at start
        0
    };

    // Combine CSS and script for injection
    let combined_injection = format!("{}\n{}", default_css, script);

    if injection_point == 0 {
        // Prepend both CSS and script
        let mut result = combined_injection;
        result.push_str(&html_str);
        result.into_bytes()
    } else {
        // Insert at injection point
        let mut result = String::new();
        result.push_str(&html_str[..injection_point]);
        result.push_str(&combined_injection);
        result.push_str(&html_str[injection_point..]);
        result.into_bytes()
    }
}

fn load_books_from(directory: std::path::PathBuf) -> HashMap<String, EpubDoc<BufReader<File>>>{
    let mut books = HashMap::new();

    if let Ok(entries) = directory.read_dir() {
        for file in entries {
            if let Ok(file) = file {
                if let Ok(_md) = file.metadata() {
                    let file_name = file.file_name().into_string().unwrap_or_default();
                    // Only process .epub files
                    if file_name.ends_with(".epub") {
                        match EpubDoc::new(file.path()) {
                            Ok(doc) => {
                                println!("✓ Loaded: {}", file_name);
                                books.insert(file_name, doc);
                            }
                            Err(e) => {
                                eprintln!("✗ Failed to load {}: {}", file_name, e);
                            }
                        }
                    }
                }
            }
        }
    }

    books
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {

    let books = Arc::new(Mutex::new(load_books_from(std::path::PathBuf::from("/Users/richardwooding/books"))));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(LibraryState(books.clone()))
        .register_asynchronous_uri_scheme_protocol("epub",  move |_ctx, request, responder| {
            let books = Arc::clone(&books);
            let host = request.uri().host().unwrap().to_string();
            // skip leading `/`
            let path = request.uri().path()[1..].to_string();

            std::thread::spawn(move || {
                let mut books_guard = books.lock().unwrap();
                let book_result = books_guard.get_mut(&host);
                match book_result {
                    Some(book) => {
                        let mime: String;
                        match book.get_resource_mime_by_path(&path) {
                            Ok(found_mime) => {
                                mime = found_mime
                            }
                            Err(_e) => {
                                responder.respond(ResponseBuilder::new().status(404).body(Vec::new()).unwrap());
                                return
                            }
                        }
                        match book.get_resource_by_path(&path) {
                            Ok(resource) => {
                                // Check if content is HTML/XHTML
                                let should_inject_script = mime == "text/html"
                                    || mime == "application/xhtml+xml"
                                    || mime == "application/xhtml"
                                    || mime == "text/xhtml";

                                let final_body = if should_inject_script {
                                    inject_link_handler_script(resource)
                                } else {
                                    resource
                                };

                                responder.respond(
                                    ResponseBuilder::new()
                                        .status(200)
                                        .header("Content-Type", &mime)
                                        .body(final_body)
                                        .unwrap()
                                )
                            }
                            Err(_e) => {
                                responder.respond(ResponseBuilder::new().status(404).body(Vec::new()).unwrap())
                            }
                        }
                    }
                    None => {
                        responder.respond(ResponseBuilder::new().status(404).body(Vec::new()).unwrap())
                    }

                }
            });
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            all_book_covers,
            get_book_title,
            get_book_toc,
            get_spine,
            get_current_spine_index,
            get_spine_item
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
