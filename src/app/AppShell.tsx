import { NavLink, Outlet } from "react-router-dom";

const navigation = [
  { to: "/", label: "Genel Bakış", short: "GB", end: true },
  { to: "/pipeline", label: "Pipeline", short: "PL" },
  { to: "/leads", label: "Leadler", short: "LD" },
  { to: "/analytics", label: "Analiz", short: "AN" },
  { to: "/imports", label: "İçe Aktarımlar", short: "IA" },
  { to: "/settings", label: "Ayarlar", short: "AY" },
];

export function AppShell() {
  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand-block">
          <div className="brand-mark">E</div>
          <div>
            <div className="brand-name">Ertip Lead Manager</div>
            <div className="brand-subtitle">Yerel satış çalışma alanı</div>
          </div>
        </div>

        <nav className="nav-list" aria-label="Ana menü">
          {navigation.map((item) => (
            <NavLink
              key={item.to}
              to={item.to}
              end={item.end}
              className={({ isActive }) => `nav-item${isActive ? " nav-item-active" : ""}`}
            >
              <span className="nav-icon" aria-hidden="true">
                {item.short}
              </span>
              <span>{item.label}</span>
            </NavLink>
          ))}
        </nav>

        <div className="sidebar-footer">
          <span className="status-dot" aria-hidden="true" />
          <span>Yerel mod · v0.1.0</span>
        </div>
      </aside>

      <main className="main-area">
        <header className="topbar">
          <div>
            <div className="eyebrow">ERTIP MEDICAL</div>
            <div className="topbar-title">Lead Yönetimi</div>
          </div>
          <div className="local-badge">Çevrimdışı hazır</div>
        </header>
        <div className="page-scroll">
          <Outlet />
        </div>
      </main>
    </div>
  );
}
