import { BigFile, formatBytes, formatCount, TypeSlice, typeColor } from "../lib/types";

interface Props {
  largest: BigFile[];
  types: TypeSlice[];
}

export default function Sidebar({ largest, types }: Props) {
  const maxTypeFrac = types.length ? Math.max(...types.map((t) => t.frac)) : 1;

  return (
    <aside className="sidebar">
      <section>
        <h2 className="section-label">By type</h2>
        <div className="type-list">
          {types.map((slice) => (
            <div className="type-row" key={slice.ext}>
              <span className="type-ext">
                <i className="swatch" style={{ background: typeColor(slice.ext) }} />
                .{slice.ext}
              </span>
              <div className="type-bar">
                <div
                  className="type-fill"
                  style={{
                    width: `${(slice.frac / maxTypeFrac) * 100}%`,
                    background: typeColor(slice.ext),
                  }}
                />
              </div>
              <span className="type-bytes">{formatBytes(slice.bytes)}</span>
            </div>
          ))}
          {types.length === 0 && <p className="empty">scan a folder to see the breakdown</p>}
        </div>
      </section>

      <section className="grow">
        <h2 className="section-label">Largest files</h2>
        <ol className="big-list">
          {largest.map((file, index) => (
            <li key={file.id} title={file.path}>
              <span className="rank">{String(index + 1).padStart(2, "0")}</span>
              <span className="big-name">
                {file.name}
                <span className="big-path">{file.path}</span>
              </span>
              <span className="big-size">{formatBytes(file.size)}</span>
            </li>
          ))}
        </ol>
        {largest.length === 0 && <p className="empty">nothing yet</p>}
      </section>

      {largest.length > 0 && (
        <footer className="sidebar-foot">
          top {formatCount(largest.length)} files ·{" "}
          {formatBytes(largest.reduce((sum, file) => sum + file.size, 0))} combined
        </footer>
      )}
    </aside>
  );
}
