export function PipelinePage() {
  return (
    <section className="page-stack">
      <div className="page-heading">
        <div>
          <div className="eyebrow">LIFECYCLE</div>
          <h1>Pipeline</h1>
          <p>NEW → CONTACTED → REPLIED → QUALIFIED → QUOTE_SENT → WON/LOST.</p>
        </div>
      </div>
      <article className="panel empty-state">
        <div className="empty-icon">PL</div>
        <h2>Kanban görünümü M4'te etkinleşecek</h2>
        <p>Status değişiklikleri aynı domain servisini kullanacak ve aktivite kaydı oluşturacak.</p>
      </article>
    </section>
  );
}
