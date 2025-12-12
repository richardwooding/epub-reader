import { useState, useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import "./BookLibrary.css";

function BookLibrary() {
  const navigate = useNavigate();
  const [books, setBooks] = useState<[string, string, string][]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    loadBookCovers();
  }, []);

  async function loadBookCovers() {
    try {
      setLoading(true);
      setError(null);
      // Call the Rust command
      const bookCovers = await invoke<[string, string, string][]>("all_book_covers");
      setBooks(bookCovers);
    } catch (err) {
      setError(`Failed to load books: ${err}`);
      console.error("Error loading book covers:", err);
    } finally {
      setLoading(false);
    }
  }

  if (loading) {
    return (
      <div className="book-library">
        <h2>My Library</h2>
        <div className="loading">Loading books...</div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="book-library">
        <h2>My Library</h2>
        <div className="error">{error}</div>
        <button onClick={loadBookCovers}>Retry</button>
      </div>
    );
  }

  return (
    <div className="book-library">
      <h2>My Library</h2>
      {books.length === 0 ? (
        <div className="empty-state">
          <p>No books found in your library.</p>
          <p>Add EPUB files to ~/books to get started.</p>
        </div>
      ) : (
        <div className="book-grid">
          {books.map(([bookKey, bookTitle, coverUri]) => {
            const handleBookClick = () => {
              navigate(`/book/${bookKey}`);
            };

            return (
              <div key={bookKey} className="book-card" onClick={handleBookClick}>
                <div className="book-cover">
                  <img
                    src={coverUri}
                    alt={`${bookTitle} cover`}
                    onError={(e) => {
                      // Fallback to placeholder if image fails to load
                      (e.target as HTMLImageElement).src =
                        "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='300' height='450'%3E%3Crect width='300' height='450' fill='%23e0e0e0'/%3E%3Ctext x='50%25' y='50%25' dominant-baseline='middle' text-anchor='middle' font-family='sans-serif' font-size='18' fill='%23999'%3ENo Cover%3C/text%3E%3C/svg%3E";
                    }}
                  />
                </div>
                <div className="book-info">
                  <h3 className="book-title">{bookTitle}</h3>
                  <span className="book-key">{bookKey}</span>
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

export default BookLibrary;
