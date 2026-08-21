const cards = [
  ["Toplam Lead", "—", "M2 import sonrası aktif"],
  ["Yeni", "—", "İletişim bekleyen"],
  ["Qualified", "—", "Nitelikli lead"],
  ["Teklif", "—", "QUOTE_SENT"],
  ["Kazanılan", "—", "WON"],
  ["Dönüşüm", "—", "M5 metriği"],
];

export function DashboardPage() {
  return (
    <section className="page-stack">
      <div className="page-heading">
        <div>
          <div className="eyebrow">ÇALIŞMA ALANI</div>
          <h1>Genel Bakış</h1>
          <p>M1 temel kabuk hazır. Gerçek metrikler veri içe aktarma tamamlandığında bağlanacak.</p>
        </div>
        <button className="primary-button" type="button" disabled>
          Excel İçe Aktar · M2
        </button>
      </div>

      <div className="kpi-grid">
        {cards.map(([title, value, hint]) => (
          <article className="kpi-card" key={title}>
            <div className="kpi-label">{title}</div>
            <div className="kpi-value">{value}</div>
            <div className="kpi-hint">{hint}</div>
          </article>
        ))}
      </div>

      <div className="content-grid">
        <article className="panel panel-large">
          <div className="panel-heading">
            <div>
              <h2>Lead Akışı</h2>
              <p>Trend grafiği M5 aşamasında gerçek sorgulara bağlanacak.</p>
            </div>
            <span className="placeholder-pill">Henüz veri yok</span>
          </div>
          <div className="chart-placeholder">
            <span>Analytics placeholder</span>
          </div>
        </article>
        <article className="panel">
          <div className="panel-heading">
            <div>
              <h2>İlgilenilmesi Gerekenler</h2>
              <p>Follow-up ve veri kalitesi özetleri burada görünecek.</p>
            </div>
          </div>
          <div className="empty-list">M4 sonrası aksiyon listesi aktif olacak.</div>
        </article>
      </div>
    </section>
  );
}
