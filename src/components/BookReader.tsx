import { useState, useEffect, useRef } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { TocItem, ReadingPosition } from "../types/book";
import TableOfContents from "./TableOfContents";
import IframeViewer, { IframeViewerRef } from "./IframeViewer";
import "./BookReader.css";

function BookReader() {
  const { bookKey } = useParams<{ bookKey: string }>();
  const navigate = useNavigate();

  const [toc, setToc] = useState<TocItem[]>([]);
  const [currentContent, setCurrentContent] = useState<string>("");
  const [bookTitle, setBookTitle] = useState<string>("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Pagination state
  const [currentPage, setCurrentPage] = useState<number>(0);
  const [totalPages, setTotalPages] = useState<number>(0);

  // Spine state for cross-chapter navigation
  const [spine, setSpine] = useState<string[]>([]);
  const [currentSpineIndex, setCurrentSpineIndex] = useState<number>(0);

  // Ref for iframe communication
  const iframeRef = useRef<IframeViewerRef>(null);

  useEffect(() => {
    if (!bookKey) {
      setError("No book specified");
      setLoading(false);
      return;
    }

    loadBook();
  }, [bookKey]);

  // LocalStorage functions for reading position persistence
  function saveReadingPosition() {
    if (!bookKey || !currentContent) return;

    const position: ReadingPosition = {
      bookKey,
      contentPath: currentContent.replace(`epub://${bookKey}/`, ''),
      page: currentPage,
      timestamp: Date.now()
    };

    localStorage.setItem(`reading-${bookKey}`, JSON.stringify(position));
  }

  function loadReadingPosition(): ReadingPosition | null {
    if (!bookKey) return null;

    const saved = localStorage.getItem(`reading-${bookKey}`);
    if (!saved) return null;

    try {
      return JSON.parse(saved) as ReadingPosition;
    } catch {
      return null;
    }
  }

  async function loadBook() {
    try {
      setLoading(true);
      setError(null);

      // Fetch the real book title from metadata
      const title = await invoke<string>("get_book_title", { bookKey });
      setBookTitle(title);

      // Fetch the table of contents
      const tocData = await invoke<TocItem[]>("get_book_toc", { bookKey });
      setToc(tocData);

      // Fetch spine for sequential navigation
      const spineData = await invoke<string[]>("get_spine", { bookKey });
      setSpine(spineData);

      // Check for saved reading position
      const savedPosition = loadReadingPosition();

      if (savedPosition && savedPosition.bookKey === bookKey) {
        // Resume from saved position
        const contentPath = savedPosition.contentPath;
        setCurrentContent(`epub://${bookKey}/${contentPath}`);

        // Find position in spine
        const spineIdx = await invoke<number | null>("get_current_spine_index", {
          bookKey,
          contentPath
        });
        if (spineIdx !== null) {
          setCurrentSpineIndex(spineIdx);
        }
      } else if (spineData.length > 0) {
        // Start from first spine item (canonical beginning)
        const firstContent = spineData[0];
        setCurrentContent(`epub://${bookKey}/${firstContent}`);
        setCurrentSpineIndex(0);
      }
    } catch (err) {
      setError(`Failed to load book: ${err}`);
      console.error("Error loading book:", err);
    } finally {
      setLoading(false);
    }
  }

  // Pagination handlers
  function handlePaginationUpdate(page: number, total: number) {
    setCurrentPage(page);
    setTotalPages(total);
    saveReadingPosition(); // Persist on every page change
  }

  async function handleNextPage() {
    // Check if we're on the last page of current chapter
    if (currentPage >= totalPages - 1) {
      // Load next chapter from spine
      if (currentSpineIndex < spine.length - 1) {
        const nextSpineIndex = currentSpineIndex + 1;
        const nextContentPath = spine[nextSpineIndex];

        setCurrentContent(`epub://${bookKey}/${nextContentPath}`);
        setCurrentSpineIndex(nextSpineIndex);
        setCurrentPage(0); // Reset to first page of new chapter
      }
      // If already on last chapter's last page, do nothing
    } else {
      // Navigate to next page within current chapter
      iframeRef.current?.sendMessage({ type: 'pagination-next' });
    }
  }

  async function handlePreviousPage() {
    // Check if we're on the first page of current chapter
    if (currentPage === 0) {
      // Load previous chapter from spine
      if (currentSpineIndex > 0) {
        const prevSpineIndex = currentSpineIndex - 1;
        const prevContentPath = spine[prevSpineIndex];

        setCurrentContent(`epub://${bookKey}/${prevContentPath}`);
        setCurrentSpineIndex(prevSpineIndex);
        // Will start at page 0 of previous chapter
        setCurrentPage(0);
      }
      // If already on first chapter's first page, do nothing
    } else {
      // Navigate to previous page within current chapter
      iframeRef.current?.sendMessage({ type: 'pagination-previous' });
    }
  }

  async function handleTocItemClick(content: string) {
    const fullUri = `epub://${bookKey}/${content}`;
    setCurrentContent(fullUri);
    setCurrentPage(0); // Reset to first page of new chapter

    // Update spine index when manually navigating via TOC
    try {
      const spineIdx = await invoke<number | null>("get_current_spine_index", {
        bookKey,
        contentPath: content
      });
      if (spineIdx !== null) {
        setCurrentSpineIndex(spineIdx);
      }
    } catch (err) {
      console.error("Failed to get spine index:", err);
    }
  }

  function handleBackClick() {
    navigate("/");
  }

  if (loading) {
    return (
      <div className="book-reader">
        <div className="reader-loading">Loading book...</div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="book-reader">
        <div className="reader-error">
          <p>{error}</p>
          <button onClick={handleBackClick}>← Back to Library</button>
        </div>
      </div>
    );
  }

  return (
    <div className="book-reader">
      <header className="reader-header">
        <button className="back-button" onClick={handleBackClick}>
          ← Library
        </button>
        <h1 className="book-title-header">{bookTitle}</h1>

        <div className="pagination-controls">
          <button
            className="page-nav-button"
            onClick={handlePreviousPage}
            disabled={currentPage === 0 && currentSpineIndex === 0}
            title={currentPage === 0 && currentSpineIndex > 0 ? "Previous chapter" : "Previous page"}
          >
            ◀
          </button>
          <span className="page-indicator">
            Page {currentPage + 1} of {totalPages}
          </span>
          <button
            className="page-nav-button"
            onClick={handleNextPage}
            disabled={currentPage >= totalPages - 1 && currentSpineIndex >= spine.length - 1}
            title={currentPage >= totalPages - 1 && currentSpineIndex < spine.length - 1 ? "Next chapter" : "Next page"}
          >
            ▶
          </button>
        </div>
      </header>

      <div className="progress-bar-container">
        <div
          className="progress-bar-fill"
          style={{
            width: `${totalPages > 0 ? ((currentPage + 1) / totalPages) * 100 : 0}%`
          }}
        />
      </div>

      <div className="reader-content">
        <TableOfContents
          toc={toc}
          onItemClick={handleTocItemClick}
        />
        <div className="content-viewer">
          {currentContent ? (
            <IframeViewer
              ref={iframeRef}
              uri={currentContent}
              title={bookTitle}
              width="100%"
              height="100%"
              onPaginationUpdate={handlePaginationUpdate}
            />
          ) : (
            <div className="no-content">Select a chapter to begin reading</div>
          )}
        </div>
      </div>
    </div>
  );
}

export default BookReader;
