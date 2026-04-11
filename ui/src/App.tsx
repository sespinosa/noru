import { NavLink, Route, Routes, Navigate } from "react-router-dom";
import TranscriptList from "./views/TranscriptList";
import TranscriptViewer from "./views/TranscriptViewer";
import Settings from "./views/Settings";

export default function App() {
  return (
    <div className="app">
      <div className="app-header">noru</div>
      <nav className="nav">
        <NavLink to="/transcripts" className={({ isActive }) => (isActive ? "active" : "")}>
          Transcripts
        </NavLink>
        <NavLink to="/settings" className={({ isActive }) => (isActive ? "active" : "")}>
          Settings
        </NavLink>
      </nav>
      <div className="app-body">
        <Routes>
          <Route path="/" element={<Navigate to="/transcripts" replace />} />
          <Route
            path="/transcripts"
            element={
              <>
                <aside className="sidebar">
                  <TranscriptList />
                </aside>
                <main className="main">
                  <p className="placeholder">Select a transcript from the sidebar.</p>
                </main>
              </>
            }
          />
          <Route
            path="/transcripts/:id"
            element={
              <>
                <aside className="sidebar">
                  <TranscriptList />
                </aside>
                <main className="main">
                  <TranscriptViewer />
                </main>
              </>
            }
          />
          <Route
            path="/settings"
            element={
              <main className="main">
                <Settings />
              </main>
            }
          />
        </Routes>
      </div>
    </div>
  );
}
