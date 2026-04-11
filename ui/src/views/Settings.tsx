export default function Settings() {
  return (
    <div>
      <h2>Settings</h2>
      <section className="settings-section">
        <h3>General</h3>
        <p className="placeholder">Autostart, transcripts directory, theme. (Phase 2)</p>
      </section>
      <section className="settings-section">
        <h3>Recording</h3>
        <p className="placeholder">Meeting platforms, audio devices. (Phase 2)</p>
      </section>
      <section className="settings-section">
        <h3>Whisper</h3>
        <p className="placeholder">Model selection and language. (Phase 2)</p>
      </section>
      <section className="settings-section">
        <h3>AI Features (experimental)</h3>
        <p className="placeholder">Sign in with ChatGPT. (Phase 2)</p>
      </section>
    </div>
  );
}
