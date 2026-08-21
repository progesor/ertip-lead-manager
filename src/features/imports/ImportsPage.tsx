export function ImportsPage() {
  return (
    <section className="page-stack">
      <div className="page-heading">
        <div>
          <div className="eyebrow">DATA INGESTION</div>
          <h1>İçe Aktarımlar</h1>
          <p>Preview-first Excel import, duplicate/repeat detection ve import geçmişi M2 kapsamındadır.</p>
        </div>
      </div>
      <article className="panel empty-state">
        <div className="empty-icon">IA</div>
        <h2>Manuel Excel import için altyapı ayrıldı</h2>
        <p>Yeni Meta multi-select export formatı gerçek dosya ile doğrulanmadan delimiter tahmini yapılmayacak.</p>
      </article>
    </section>
  );
}
