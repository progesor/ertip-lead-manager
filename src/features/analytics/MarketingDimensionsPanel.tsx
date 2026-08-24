import { useMemo, useState } from "react";
import type { AnalyticsNamedBreakdownPoint } from "./types";
import "./marketing-dimensions.css";

type DimensionKey = "campaign" | "form" | "adset" | "ad";

interface Props {
  campaigns: AnalyticsNamedBreakdownPoint[];
  forms: AnalyticsNamedBreakdownPoint[];
  adsets: AnalyticsNamedBreakdownPoint[];
  ads: AnalyticsNamedBreakdownPoint[];
}

const labels: Record<DimensionKey, string> = {
  campaign: "Kampanya",
  form: "Form",
  adset: "Ad Set",
  ad: "Reklam",
};

export function MarketingDimensionsPanel({ campaigns, forms, adsets, ads }: Props) {
  const [dimension, setDimension] = useState<DimensionKey>("campaign");
  const [search, setSearch] = useState("");

  const source =
    dimension === "campaign"
      ? campaigns
      : dimension === "form"
        ? forms
        : dimension === "adset"
          ? adsets
          : ads;

  const filtered = useMemo(() => {
    const query = search.trim().toLocaleLowerCase("tr-TR");
    if (!query) return source;
    return source.filter((item) =>
      `${item.name} ${item.key}`.toLocaleLowerCase("tr-TR").includes(query),
    );
  }, [search, source]);

  const visible = filtered.slice(0, 25);
  const max = Math.max(...visible.map((item) => item.submissions), 1);

  return (
    <article className="panel marketing-dimensions-panel">
      <div className="panel-heading marketing-dimensions-heading">
        <div>
          <h2>Kaynak Performansı</h2>
          <p>Kampanya, form, ad set ve reklamları Meta ID'sini kaybetmeden karşılaştır.</p>
        </div>
        <span className="placeholder-pill">{source.length} kayıt</span>
      </div>

      <div className="marketing-dimensions-toolbar">
        <fieldset className="marketing-dimension-switcher">
          <legend>Kaynak kırılımı</legend>
          {(Object.keys(labels) as DimensionKey[]).map((key) => (
            <button
              type="button"
              key={key}
              className={dimension === key ? "is-active" : ""}
              aria-pressed={dimension === key}
              onClick={() => {
                setDimension(key);
                setSearch("");
              }}
            >
              {labels[key]}
            </button>
          ))}
        </fieldset>

        <label className="marketing-dimensions-search">
          <span>{labels[dimension]} ara</span>
          <input
            type="search"
            value={search}
            onChange={(event) => setSearch(event.target.value)}
            placeholder="Ad veya Meta ID"
          />
        </label>
      </div>

      {visible.length > 0 ? (
        <div className="marketing-dimensions-table-wrap">
          <table className="marketing-dimensions-table">
            <thead>
              <tr>
                <th>{labels[dimension]}</th>
                <th>Submission</th>
                <th>Benzersiz Kişi</th>
                <th>Dağılım</th>
              </tr>
            </thead>
            <tbody>
              {visible.map((item) => (
                <tr key={item.key}>
                  <td>
                    <strong>{item.name}</strong>
                    <span>{item.key}</span>
                  </td>
                  <td>{item.submissions.toLocaleString("tr-TR")}</td>
                  <td>{item.uniqueContacts.toLocaleString("tr-TR")}</td>
                  <td>
                    <div className="marketing-dimension-track">
                      <span style={{ width: `${(item.submissions / max) * 100}%` }} />
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : (
        <div className="analytics-empty">
          {search ? "Aramayla eşleşen kayıt yok." : "Bu kırılım için veri yok."}
        </div>
      )}

      {filtered.length > visible.length ? (
        <div className="marketing-dimensions-more">
          İlk {visible.length} / {filtered.length} kayıt gösteriliyor. Aramayla daraltabilirsiniz.
        </div>
      ) : null}
    </article>
  );
}
