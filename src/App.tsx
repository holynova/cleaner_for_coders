import { FormEvent, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./App.css";

type Lang = "zh" | "en";
type SortBy = "size" | "last_used";
type NodeModulesMode = "stale" | "all";

interface DockerOptions {
  dangling_images: boolean;
  unused_images: boolean;
  stopped_containers: boolean;
  build_cache: boolean;
  volumes: boolean;
}

interface ScanRequest {
  scan_paths: string[];
  ignore_patterns: string[];
  sort_by: SortBy;
  node_modules_mode: NodeModulesMode;
  stale_days: number;
  docker_options: DockerOptions;
}

interface CleanupItem {
  id: string;
  kind: string;
  path: string;
  size_bytes: number;
  last_used_unix: number;
  risk: string;
  source: string;
}

interface ScanResponse {
  total_bytes: number;
  item_count: number;
  items: CleanupItem[];
}

interface CleanupResponse {
  freed_bytes: number;
  success_count: number;
  failed_count: number;
}

interface LogPayload {
  level: string;
  message: string;
}

const text = {
  zh: {
    title: "Dev 空间清理工具",
    subtitle: "扫描可清理开发垃圾，按体积和最近使用时间排序，安全清理",
    language: "English",
    scan: "开始扫描",
    scanning: "扫描中...",
    cleanup: "清理选中项",
    cleaning: "清理中...",
    selectAll: "全选",
    clearSel: "清空",
    sortBy: "排序方式",
    bySize: "按大小",
    byLastUsed: "按最近使用",
    nodeMode: "node_modules 策略",
    staleOnly: "仅旧目录",
    allNodes: "全部",
    staleDays: "旧目录天数",
    advanced: "高级选项",
    customPaths: "扫描路径（默认全盘，多个路径用换行或逗号分隔）",
    ignorePatterns: "忽略规则（路径片段，多个用换行或逗号分隔）",
    dockerScope: "Docker 清理范围（可多选）",
    dangling: "悬空镜像（默认）",
    unusedImages: "未使用镜像",
    stoppedContainers: "已停止容器",
    buildCache: "构建缓存",
    volumes: "数据卷",
    est: "预计可释放",
    items: "候选项",
    selected: "已选",
    type: "类型",
    path: "路径",
    size: "大小",
    lastUsed: "最近使用",
    risk: "风险提示",
    never: "未知",
    empty: "暂无结果，请先扫描",
    confirm:
      "即将删除选中项。删除 node_modules 后需要重新安装依赖，删除 Docker 数据可能影响容器运行。确认继续？",
    report: "清理结果",
    logs: "实时日志",
  },
  en: {
    title: "Dev Space Cleaner",
    subtitle:
      "Scan removable developer artifacts, sort by size/recent use, and clean safely",
    language: "中文",
    scan: "Start Scan",
    scanning: "Scanning...",
    cleanup: "Clean Selected",
    cleaning: "Cleaning...",
    selectAll: "Select all",
    clearSel: "Clear",
    sortBy: "Sort by",
    bySize: "Size",
    byLastUsed: "Last used",
    nodeMode: "node_modules policy",
    staleOnly: "Stale only",
    allNodes: "All",
    staleDays: "Stale days",
    advanced: "Advanced",
    customPaths: "Scan paths (default full disk, newline/comma separated)",
    ignorePatterns: "Ignore rules (path tokens, newline/comma separated)",
    dockerScope: "Docker cleanup scope (multi-select)",
    dangling: "Dangling images (default)",
    unusedImages: "Unused images",
    stoppedContainers: "Stopped containers",
    buildCache: "Build cache",
    volumes: "Volumes",
    est: "Estimated reclaimable",
    items: "Candidates",
    selected: "Selected",
    type: "Kind",
    path: "Path",
    size: "Size",
    lastUsed: "Last used",
    risk: "Risk",
    never: "Unknown",
    empty: "No results yet. Start a scan first.",
    confirm:
      "Selected targets will be deleted. node_modules removal requires reinstalling dependencies and Docker cleanup may impact containers. Continue?",
    report: "Cleanup report",
    logs: "Live logs",
  },
} as const;

function toList(raw: string): string[] {
  return raw
    .split(/[\n,]/)
    .map((v) => v.trim())
    .filter(Boolean);
}

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const base = 1024;
  const index = Math.min(Math.floor(Math.log(bytes) / Math.log(base)), units.length - 1);
  const value = bytes / base ** index;
  return `${value.toFixed(value >= 10 || index === 0 ? 0 : 1)} ${units[index]}`;
}

function formatTime(ts: number, lang: Lang, fallback: string): string {
  if (!ts) return fallback;
  const locale = lang === "zh" ? "zh-CN" : "en-US";
  return new Date(ts * 1000).toLocaleString(locale);
}

function App() {
  const [lang, setLang] = useState<Lang>("zh");
  const [sortBy, setSortBy] = useState<SortBy>("size");
  const [nodeMode, setNodeMode] = useState<NodeModulesMode>("stale");
  const [staleDays, setStaleDays] = useState(30);
  const [customPaths, setCustomPaths] = useState("");
  const [ignorePatterns, setIgnorePatterns] = useState("");
  const [docker, setDocker] = useState<DockerOptions>({
    dangling_images: true,
    unused_images: false,
    stopped_containers: false,
    build_cache: false,
    volumes: false,
  });

  const [items, setItems] = useState<CleanupItem[]>([]);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [totalBytes, setTotalBytes] = useState(0);
  const [scanning, setScanning] = useState(false);
  const [cleaning, setCleaning] = useState(false);
  const [report, setReport] = useState<CleanupResponse | null>(null);
  const [logs, setLogs] = useState<string[]>([]);

  const t = text[lang];

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    listen<LogPayload>("cleanup-log", (event) => {
      const payload = event.payload;
      setLogs((prev) => [
        ...prev,
        `[${new Date().toLocaleTimeString()}][${payload.level}] ${payload.message}`,
      ]);
    }).then((fn) => {
      unlisten = fn;
    });

    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  const selectedItems = useMemo(
    () => items.filter((item) => selectedIds.has(item.id)),
    [items, selectedIds],
  );

  const selectedBytes = useMemo(
    () => selectedItems.reduce((sum, item) => sum + item.size_bytes, 0),
    [selectedItems],
  );

  async function startScan(e?: FormEvent) {
    e?.preventDefault();
    setScanning(true);
    setReport(null);
    setLogs([]);

    const request: ScanRequest = {
      scan_paths: toList(customPaths),
      ignore_patterns: toList(ignorePatterns),
      sort_by: sortBy,
      node_modules_mode: nodeMode,
      stale_days: staleDays,
      docker_options: docker,
    };

    try {
      const result = await invoke<ScanResponse>("scan_cleanup_targets", { payload: request });
      setItems(result.items);
      setTotalBytes(result.total_bytes);
      setSelectedIds(new Set());
    } catch (error) {
      setLogs((prev) => [...prev, `[error] ${String(error)}`]);
    } finally {
      setScanning(false);
    }
  }

  async function doCleanup() {
    if (selectedItems.length === 0 || cleaning) return;
    if (!window.confirm(t.confirm)) return;

    setCleaning(true);
    setReport(null);
    try {
      const result = await invoke<CleanupResponse>("execute_cleanup", {
        payload: { items: selectedItems },
      });
      setReport(result);
      setItems((prev) => prev.filter((item) => !selectedIds.has(item.id)));
      setSelectedIds(new Set());
      setTotalBytes((prev) => Math.max(0, prev - result.freed_bytes));
    } catch (error) {
      setLogs((prev) => [...prev, `[error] ${String(error)}`]);
    } finally {
      setCleaning(false);
    }
  }

  const allSelected = items.length > 0 && selectedIds.size === items.length;

  return (
    <main className="app-shell">
      <header className="hero">
        <div>
          <h1>{t.title}</h1>
          <p>{t.subtitle}</p>
        </div>
        <button className="lang-btn" onClick={() => setLang((v) => (v === "zh" ? "en" : "zh"))}>
          {t.language}
        </button>
      </header>

      <section className="panel">
        <form className="controls" onSubmit={startScan}>
          <label>
            {t.sortBy}
            <select value={sortBy} onChange={(e) => setSortBy(e.target.value as SortBy)}>
              <option value="size">{t.bySize}</option>
              <option value="last_used">{t.byLastUsed}</option>
            </select>
          </label>

          <label>
            {t.nodeMode}
            <select value={nodeMode} onChange={(e) => setNodeMode(e.target.value as NodeModulesMode)}>
              <option value="stale">{t.staleOnly}</option>
              <option value="all">{t.allNodes}</option>
            </select>
          </label>

          <label>
            {t.staleDays}
            <input
              type="number"
              min={1}
              value={staleDays}
              onChange={(e) => setStaleDays(Number(e.target.value) || 30)}
            />
          </label>

          <button className="primary" type="submit" disabled={scanning}>
            {scanning ? t.scanning : t.scan}
          </button>
        </form>

        <details className="advanced">
          <summary>{t.advanced}</summary>
          <div className="advanced-grid">
            <label>
              {t.customPaths}
              <textarea
                value={customPaths}
                onChange={(e) => setCustomPaths(e.target.value)}
                placeholder={lang === "zh" ? "/Users/sym/Code\n/D/workspace" : "/Users/sym/Code\nD:\\workspace"}
              />
            </label>
            <label>
              {t.ignorePatterns}
              <textarea
                value={ignorePatterns}
                onChange={(e) => setIgnorePatterns(e.target.value)}
                placeholder={lang === "zh" ? ".git\nnode_modules/.cache" : ".git\nnode_modules/.cache"}
              />
            </label>
            <fieldset>
              <legend>{t.dockerScope}</legend>
              <label>
                <input
                  type="checkbox"
                  checked={docker.dangling_images}
                  onChange={(e) => setDocker((d) => ({ ...d, dangling_images: e.target.checked }))}
                />
                {t.dangling}
              </label>
              <label>
                <input
                  type="checkbox"
                  checked={docker.unused_images}
                  onChange={(e) => setDocker((d) => ({ ...d, unused_images: e.target.checked }))}
                />
                {t.unusedImages}
              </label>
              <label>
                <input
                  type="checkbox"
                  checked={docker.stopped_containers}
                  onChange={(e) =>
                    setDocker((d) => ({ ...d, stopped_containers: e.target.checked }))
                  }
                />
                {t.stoppedContainers}
              </label>
              <label>
                <input
                  type="checkbox"
                  checked={docker.build_cache}
                  onChange={(e) => setDocker((d) => ({ ...d, build_cache: e.target.checked }))}
                />
                {t.buildCache}
              </label>
              <label>
                <input
                  type="checkbox"
                  checked={docker.volumes}
                  onChange={(e) => setDocker((d) => ({ ...d, volumes: e.target.checked }))}
                />
                {t.volumes}
              </label>
            </fieldset>
          </div>
        </details>
      </section>

      <section className="panel metrics">
        <div>
          <strong>{t.est}</strong>
          <p>{formatBytes(totalBytes)}</p>
        </div>
        <div>
          <strong>{t.items}</strong>
          <p>{items.length}</p>
        </div>
        <div>
          <strong>{t.selected}</strong>
          <p>
            {selectedItems.length} / {formatBytes(selectedBytes)}
          </p>
        </div>
        <div className="actions">
          <button
            onClick={() =>
              setSelectedIds(allSelected ? new Set() : new Set(items.map((item) => item.id)))
            }
            disabled={items.length === 0}
          >
            {allSelected ? t.clearSel : t.selectAll}
          </button>
          <button className="danger" onClick={doCleanup} disabled={selectedItems.length === 0 || cleaning}>
            {cleaning ? t.cleaning : t.cleanup}
          </button>
        </div>
      </section>

      <section className="panel">
        {items.length === 0 ? (
          <p className="empty">{t.empty}</p>
        ) : (
          <div className="table-wrap">
            <table>
              <thead>
                <tr>
                  <th />
                  <th>{t.type}</th>
                  <th>{t.path}</th>
                  <th>{t.size}</th>
                  <th>{t.lastUsed}</th>
                  <th>{t.risk}</th>
                </tr>
              </thead>
              <tbody>
                {items.map((item) => (
                  <tr key={item.id}>
                    <td>
                      <input
                        type="checkbox"
                        checked={selectedIds.has(item.id)}
                        onChange={(e) => {
                          const next = new Set(selectedIds);
                          if (e.target.checked) {
                            next.add(item.id);
                          } else {
                            next.delete(item.id);
                          }
                          setSelectedIds(next);
                        }}
                      />
                    </td>
                    <td>{item.kind}</td>
                    <td className="path-cell">{item.path}</td>
                    <td>{formatBytes(item.size_bytes)}</td>
                    <td>{formatTime(item.last_used_unix, lang, t.never)}</td>
                    <td>{item.risk}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>

      <section className="panel logs">
        <h2>{t.logs}</h2>
        <pre>{logs.join("\n") || "-"}</pre>
        {report && (
          <p className="report">
            {t.report}: {report.success_count} success / {report.failed_count} failed / {formatBytes(report.freed_bytes)}
          </p>
        )}
      </section>
    </main>
  );
}

export default App;
