import { BrowserRouter, Routes, Route } from "react-router-dom";
import "./App.css";
import { BookLibrary, BookReader } from "./components";

function App() {
  return (
    <BrowserRouter>
      <main className="container">
        <Routes>
          <Route path="/" element={<BookLibrary />} />
          <Route path="/book/:bookKey" element={<BookReader />} />
        </Routes>
      </main>
    </BrowserRouter>
  );
}

export default App;
