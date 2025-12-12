import { useState, useEffect } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { TocItem } from "../types/book";
import TableOfContents from "./TableOfContents";
import IframeViewer from "./IframeViewer";
import "./BookReader.css";

function BookReader() {
  const { bookKey } = useParams<{ bookKey: string }>();
  const navigate = useNavigate();

  const [toc, setToc] = useState<TocItem[]>([]);
  const [currentContent, setCurrentContent] = useState<string>("");
  const [bookTitle, setBookTitle] = useState<string>("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!bookKey) {
      setError("No book specified");
      setLoading(false);
      return;
    }

    loadBook();
  }, [bookKey]);

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

      // Set first chapter as default content
      if (tocData.length > 0) {
        const firstContent = tocData[0].content;
        setCurrentContent(`epub://${bookKey}/${firstContent}`);
      }
    } catch (err) {
      setError(`Failed to load book: ${err}`);
      console.error("Error loading book:", err);
    } finally {
      setLoading(false);
    }
  }

  function handleTocItemClick(content: string) {
    const fullUri = `epub://${bookKey}/${content}`;
    setCurrentContent(fullUri);
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
        <div className="header-spacer"></div>
      </header>
      <div className="reader-content">
        <TableOfContents
          toc={toc}
          bookTitle={bookTitle}
          onItemClick={handleTocItemClick}
        />
        <div className="content-viewer">
          {currentContent ? (
            <IframeViewer
              uri={currentContent}
              title={bookTitle}
              width="100%"
              height="100%"
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
