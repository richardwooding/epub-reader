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
    }

    a {
        color: #66b3ff;
    }

    a:visited {
        color: #bb86fc;
    }

    a:hover {
        color: #99ccff;
    }
}
</style>"#;

    // JavaScript to inject
    let script = r#"<script>
//<![CDATA[
(function() {
    'use strict';

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

    document.addEventListener('click', handleLinkClick, true);
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
        .invoke_handler(tauri::generate_handler![greet, all_book_covers, get_book_title, get_book_toc])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
