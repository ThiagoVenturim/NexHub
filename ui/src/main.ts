import "./styles/tokens.css";

function renderApp() {
  const app = document.getElementById("app");
  if (!app) {
    return;
  }

  app.innerHTML = `
    <header class="top-bar">
      <div class="top-bar-left">
        <span class="brand">NexHub</span>
        <span class="workspace-name">[workspace]</span>
      </div>
      <div class="top-bar-center">
        <input class="search-input" type="text" placeholder="Search agents, notes, commands..." />
      </div>
      <div class="top-bar-right">
        <div class="status-badge" id="app-status">status: 0 running</div>
      </div>
    </header>

    <div class="main-content">
      <aside class="sidebar">
        <div class="sidebar-section">
          <div class="sidebar-section-title">Groups</div>
          <ul class="sidebar-list">
            <li class="sidebar-item">OSS</li>
            <li class="sidebar-item">Pessoais</li>
          </ul>
        </div>
        <div class="sidebar-section">
          <div class="sidebar-section-title">Floors</div>
          <ul class="sidebar-list">
            <li class="sidebar-item">develop</li>
          </ul>
        </div>
        <div class="sidebar-section">
          <div class="sidebar-section-title">Routines</div>
          <ul class="sidebar-list">
            <li class="sidebar-item">watchdog</li>
          </ul>
        </div>
      </aside>

      <main class="canvas" id="canvas">
        <div class="minimap" id="minimap"></div>
      </main>
    </div>

    <footer class="message-log collapsed" id="message-log">
      <div class="message-log-header" id="message-log-toggle">
        <span>Message log (collapsible): no messages</span>
        <span>toggle</span>
      </div>
    </footer>
  `;

  const toggle = document.getElementById("message-log-toggle");
  const log = document.getElementById("message-log");
  if (toggle && log) {
    toggle.addEventListener("click", () => {
      log.classList.toggle("collapsed");
    });
  }
}

renderApp();
