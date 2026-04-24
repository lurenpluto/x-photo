import { createApiClient } from "./apiClient.js";
import { createAlbumView } from "./views/albumView.js";
import { createPhotoView } from "./views/photoView.js";
import { createTaskView } from "./views/taskView.js";

const state = {
  apiBase: localStorage.getItem("xphoto_api_base") || "http://127.0.0.1:8080/rpc/v1",
  tab: "photos",
  albums: [],
  albumPhotoCounts: {},
  selectedPhotoId: null,
  selectedPhotoAlbums: [],
  photoItems: [],
  photoPage: 1,
  photoTotal: 0,
  photoFilters: {
    keyword: "",
    album_id: "",
    order: "desc",
    start_time: "",
    end_time: "",
  },
  search: {
    items: [],
    activeIndex: -1,
    history: [],
  },
  selectedTaskId: null,
  selectedTask: null,
  selectedScanJobId: null,
  taskStatusFilter: "",
  taskJobTypeFilter: "",
  taskTriggerTypeFilter: "",
  sourceById: {},
  detail: {
    active: false,
    type: null,
    tabBeforeOpen: "photos",
    current: null,
    stack: [],
    photoView: {
      scale: 1,
      tx: 0,
      ty: 0,
      dragging: false,
      dragStartX: 0,
      dragStartY: 0,
    },
  },
  albumDetail: {
    albumId: null,
    albumName: "",
    page: 1,
    total: 0,
    filters: {
      keyword: "",
      order: "desc",
      start_time: "",
      end_time: "",
    },
  },
  favorite: {
    page: 1,
    total: 0,
    pageSize: Math.min(100, Math.max(1, Number(localStorage.getItem("xphoto_favorite_page_size") || "36") || 36)),
  },
  timers: {
    taskAutoRefresh: null,
    taskTickBusy: false,
  },
};

const $ = (id) => document.getElementById(id);

const el = {
  layout: document.querySelector(".layout"),
  main: document.querySelector(".main"),
  sideNav: $("sideNav"),
  btnSidebarToggle: $("btnSidebarToggle"),
  tabPhotos: $("tab-photos"),
  tabFavorites: $("tab-favorites"),
  tabAlbums: $("tab-albums"),
  tabSettings: $("tab-settings"),
  tabStatus: $("tab-status"),

  photoKeyword: $("photoKeyword"),
  btnQuickSearch: $("btnQuickSearch"),
  searchSuggest: $("searchSuggest"),
  searchPrefixBar: $("searchPrefixBar"),
  searchExample: $("searchExample"),
  btnReloadAlbumsPhotos: $("btnReloadAlbumsPhotos"),
  latestAlbumsRow: $("latestAlbumsRow"),
  photoAlbumFilter: $("photoAlbumFilter"),
  photoOrder: $("photoOrder"),
  photoStartDate: $("photoStartDate"),
  photoEndDate: $("photoEndDate"),
  photoPageSize: $("photoPageSize"),
  photoFilterChips: $("photoFilterChips"),
  photoMasonry: $("photoMasonry"),
  photoPageHint: $("photoPageHint"),
  btnPhotoPrev: $("btnPhotoPrev"),
  btnPhotoNext: $("btnPhotoNext"),

  btnReloadAlbums: $("btnReloadAlbums"),
  albumGrid: $("albumGrid"),
  btnReloadFavorites: $("btnReloadFavorites"),
  favoritePageSize: $("favoritePageSize"),
  favoriteMasonry: $("favoriteMasonry"),
  btnFavoritePrev: $("btnFavoritePrev"),
  btnFavoriteNext: $("btnFavoriteNext"),
  favoritePageHint: $("favoritePageHint"),

  apiBase: $("apiBase"),
  btnHealth: $("btnHealth"),
  healthStatus: $("healthStatus"),
  sourceForm: $("sourceForm"),
  sourceName: $("sourceName"),
  sourcePath: $("sourcePath"),
  sourceMessage: $("sourceMessage"),
  btnReloadSources: $("btnReloadSources"),
  sourceList: $("sourceList"),
  sourceItemTpl: $("sourceItemTpl"),

  taskAutoRefreshHint: $("taskAutoRefreshHint"),
  btnTaskOverview: $("btnTaskOverview"),
  taskOverview: $("taskOverview"),
  btnTaskList: $("btnTaskList"),
  taskFilterForm: $("taskFilterForm"),
  taskJobType: $("taskJobType"),
  taskTriggerType: $("taskTriggerType"),
  taskStatus: $("taskStatus"),
  taskList: $("taskList"),
  taskDetailHint: $("taskDetailHint"),
  taskFlow: $("taskFlow"),
  taskDetail: $("taskDetail"),
  btnCancelTask: $("btnCancelTask"),
  btnRetryTask: $("btnRetryTask"),
  btnCancelScan: $("btnCancelScan"),
  btnRetryScan: $("btnRetryScan"),

  detailView: $("detailView"),
  btnDetailBack: $("btnDetailBack"),
  btnDetailUp: $("btnDetailUp"),
  detailTitle: $("detailTitle"),
  photoDetailNav: $("photoDetailNav"),
  photoDetailPage: $("photoDetailPage"),
  photoPreviewImg: $("photoPreviewImg"),
  photoZoomBadge: $("photoZoomBadge"),
  btnPhotoZoomIn: $("btnPhotoZoomIn"),
  btnPhotoZoomOut: $("btnPhotoZoomOut"),
  btnPhotoZoomReset: $("btnPhotoZoomReset"),
  btnPhotoFavorite: $("btnPhotoFavorite"),
  btnPhotoHelp: $("btnPhotoHelp"),
  photoHelpPanel: $("photoHelpPanel"),
  photoGeoText: $("photoGeoText"),
  photoGeoLink: $("photoGeoLink"),
  photoExifMeta: $("photoExifMeta"),
  albumDetailPage: $("albumDetailPage"),
  albumDetailMeta: $("albumDetailMeta"),
  albumDetailPhotos: $("albumDetailPhotos"),
  albumDetailKeyword: $("albumDetailKeyword"),
  albumDetailOrder: $("albumDetailOrder"),
  albumDetailStartDate: $("albumDetailStartDate"),
  albumDetailEndDate: $("albumDetailEndDate"),
  albumDetailPageSize: $("albumDetailPageSize"),
  btnAlbumDetailSearch: $("btnAlbumDetailSearch"),
  btnAlbumDetailPrev: $("btnAlbumDetailPrev"),
  btnAlbumDetailNext: $("btnAlbumDetailNext"),
  albumDetailPageHint: $("albumDetailPageHint"),
  toastContainer: $("toastContainer"),

  btnDrawerPrev: $("btnDrawerPrev"),
  btnDrawerNext: $("btnDrawerNext"),
  photoDetailPrimary: $("photoDetailPrimary"),
  photoDetailAdvanced: $("photoDetailAdvanced"),
  photoAlbums: $("photoAlbums"),
  photoRemark: $("photoRemark"),
  btnSaveRemark: $("btnSaveRemark"),
  photoAlbumSelect: $("photoAlbumSelect"),
  btnAddToAlbum: $("btnAddToAlbum"),
  photoDetailMsg: $("photoDetailMsg"),
};

const SEARCH_PREFIXES = ["album:", "remark:", "exif:", "camera:", "lens:", "path:", "source:", "date:"];

function loadSearchHistory() {
  try {
    const raw = localStorage.getItem("xphoto_search_history") || "[]";
    const arr = JSON.parse(raw);
    if (!Array.isArray(arr)) return [];
    return arr.map((v) => String(v || "").trim()).filter((v) => v).slice(0, 20);
  } catch (_err) {
    return [];
  }
}

function saveSearchHistory(keyword) {
  const value = String(keyword || "").trim();
  if (!value) return;
  const next = [value, ...state.search.history.filter((v) => v !== value)].slice(0, 20);
  state.search.history = next;
  localStorage.setItem("xphoto_search_history", JSON.stringify(next));
}

function escapeHtml(s) {
  return String(s)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

function hashNumber(s) {
  let h = 0;
  for (let i = 0; i < s.length; i += 1) {
    h = (h * 31 + s.charCodeAt(i)) >>> 0;
  }
  return h;
}

function photoVisualStyle(photo) {
  const seed = hashNumber(photo.id || photo.file_path || photo.file_name || "photo");
  const hueA = seed % 360;
  const hueB = (hueA + 46 + (seed % 97)) % 360;
  const height = 150 + (seed % 140);
  return {
    height,
    background: `linear-gradient(145deg, hsl(${hueA} 58% 56%), hsl(${hueB} 62% 42%))`,
  };
}

function albumVisualStyle(album) {
  const seed = hashNumber(album.id || album.name || "album");
  const hueA = (seed + 24) % 360;
  const hueB = (hueA + 70) % 360;
  return {
    background: `linear-gradient(140deg, hsl(${hueA} 58% 54%), hsl(${hueB} 62% 38%))`,
  };
}

function startOfDay(date) {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate());
}

function formatPhotoGroupLabel(dateText) {
  if (!dateText || dateText === "未知日期") return "未知日期";
  const d = new Date(`${dateText}T00:00:00`);
  if (Number.isNaN(d.getTime())) return dateText;

  const today = startOfDay(new Date());
  const target = startOfDay(d);
  const diffDays = Math.round((today.getTime() - target.getTime()) / 86400000);

  if (diffDays === 0) return "今天";
  if (diffDays === 1) return "昨天";
  if (diffDays > 1 && diffDays < 7) return "本周";
  return dateText;
}

function formatShotTime(isoText) {
  if (!isoText) return "未知";
  const t = String(isoText).replace("T", " ");
  return t.length > 19 ? t.slice(0, 19) : t;
}

function formatIsoToSecond(isoText) {
  if (!isoText) return "-";
  const d = new Date(isoText);
  if (!Number.isNaN(d.getTime())) {
    const y = d.getFullYear();
    const m = String(d.getMonth() + 1).padStart(2, "0");
    const day = String(d.getDate()).padStart(2, "0");
    const hh = String(d.getHours()).padStart(2, "0");
    const mm = String(d.getMinutes()).padStart(2, "0");
    const ss = String(d.getSeconds()).padStart(2, "0");
    return `${y}-${m}-${day} ${hh}:${mm}:${ss}`;
  }
  return String(isoText).replace("T", " ").slice(0, 19);
}

function updateSearchExample() {
  const text = el.photoKeyword.value.trim();
  if (text.includes("album:")) {
    el.searchExample.textContent = "示例: album:旅行 date:2025-12";
    return;
  }
  if (text.includes("camera:") || text.includes("lens:")) {
    el.searchExample.textContent = "示例: camera:iPhone lens:26mm";
    return;
  }
  if (text.includes("remark:")) {
    el.searchExample.textContent = "示例: remark:周末聚餐";
    return;
  }
  el.searchExample.textContent = "示例: album:旅行 camera:iPhone date:2025-12";
}

function insertSearchPrefix(prefix) {
  const current = el.photoKeyword.value;
  const suffix = current.endsWith(" ") || current.length === 0 ? "" : " ";
  el.photoKeyword.value = `${current}${suffix}${prefix}`;
  el.photoKeyword.focus();
  updateSearchExample();
}

function buildSearchSuggestions(inputText) {
  const text = String(inputText || "").trim();
  const lower = text.toLowerCase();
  const out = [];
  const used = new Set();
  const addItem = (value, meta) => {
    const k = `${value}::${meta}`;
    if (used.has(k)) return;
    used.add(k);
    out.push({ value, meta });
  };

  const matchedPrefixes = SEARCH_PREFIXES.filter((p) => !text || p.includes(lower));
  for (const prefix of matchedPrefixes) {
    addItem(prefix, "字段前缀");
  }

  if (text.length > 0) {
    const albumMatches = state.albums
      .filter((a) => String(a.name || "").toLowerCase().includes(lower))
      .slice(0, 6);
    for (const album of albumMatches) {
      addItem(`album:${album.name}`, "相册名");
    }
  }

  const historyMatches = state.search.history
    .filter((v) => !text || v.toLowerCase().includes(lower))
    .slice(0, 8);
  for (const item of historyMatches) {
    addItem(item, "最近搜索");
  }

  return out.slice(0, 14);
}

function closeSearchSuggestions() {
  state.search.items = [];
  state.search.activeIndex = -1;
  el.searchSuggest.classList.add("hidden");
  el.searchSuggest.innerHTML = "";
}

function applySearchSuggestion(value) {
  el.photoKeyword.value = value;
  el.photoKeyword.focus();
  updateSearchExample();
  closeSearchSuggestions();
}

function renderSearchSuggestions() {
  const items = buildSearchSuggestions(el.photoKeyword.value);
  const prevValue = state.search.items[state.search.activeIndex]?.value;
  state.search.items = items;
  if (!items.length) {
    closeSearchSuggestions();
    return;
  }
  const matchIdx = prevValue ? items.findIndex((it) => it.value === prevValue) : -1;
  state.search.activeIndex = matchIdx >= 0 ? matchIdx : Math.min(Math.max(state.search.activeIndex, 0), items.length - 1);
  el.searchSuggest.innerHTML = items
    .map(
      (item, idx) => `
      <button class="search-suggest-item ${idx === state.search.activeIndex ? "active" : ""}" type="button" data-search-suggest-index="${idx}">
        <span class="search-suggest-main">${escapeHtml(item.value)}</span>
        <span class="search-suggest-sub">${escapeHtml(item.meta)}</span>
      </button>
    `,
    )
    .join("");
  el.searchSuggest.classList.remove("hidden");

  el.searchSuggest.querySelectorAll("[data-search-suggest-index]").forEach((node) => {
    node.addEventListener("mousedown", (event) => {
      event.preventDefault();
      const idx = Number(node.getAttribute("data-search-suggest-index"));
      const item = state.search.items[idx];
      if (item) {
        applySearchSuggestion(item.value);
      }
    });
  });
}

function setApiBase(value) {
  state.apiBase = value.replace(/\/$/, "");
  localStorage.setItem("xphoto_api_base", state.apiBase);
}

function setSidebarCollapsed(collapsed) {
  el.layout.classList.toggle("sidebar-collapsed", collapsed);
  localStorage.setItem("xphoto_sidebar_collapsed", collapsed ? "1" : "0");
  if (el.btnSidebarToggle) {
    el.btnSidebarToggle.textContent = collapsed ? "展开" : "折叠侧栏";
    el.btnSidebarToggle.title = collapsed ? "展开侧栏" : "折叠侧栏";
  }
}

function toggleSidebarCollapsed() {
  const collapsed = !el.layout.classList.contains("sidebar-collapsed");
  setSidebarCollapsed(collapsed);
}

function dateToStartIso(dateValue) {
  if (!dateValue) return "";
  return `${dateValue}T00:00:00Z`;
}

function dateToEndIso(dateValue) {
  if (!dateValue) return "";
  return `${dateValue}T23:59:59Z`;
}

function isoToDateInput(iso) {
  if (!iso) return "";
  return iso.slice(0, 10);
}

function setHealthText(text, ok = true) {
  el.healthStatus.textContent = text;
  el.healthStatus.style.color = ok ? "var(--accent-deep)" : "var(--warning)";
}

function buildMapUrl(lat, lng) {
  return `https://www.google.com/maps?q=${encodeURIComponent(`${lat},${lng}`)}`;
}

function showToast(text, kind = "info") {
  if (!el.toastContainer) return;
  const node = document.createElement("div");
  node.className = `toast ${kind === "error" ? "error" : ""}`;
  node.textContent = text;
  el.toastContainer.appendChild(node);
  window.setTimeout(() => {
    node.remove();
  }, 2600);
}

const apiClient = createApiClient({ getBaseUrl: () => state.apiBase });
const api = apiClient.request;

function updateTabUi() {
  const tabs = ["photos", "favorites", "albums", "settings", "status"];
  for (const tab of tabs) {
    const panel = $(`tab-${tab}`);
    if (panel) {
      panel.classList.toggle("active", state.tab === tab);
    }
  }
  el.sideNav.querySelectorAll("[data-tab]").forEach((btn) => {
    btn.classList.toggle("active", btn.dataset.tab === state.tab);
  });
}

function openDetailView(type, title, payload, options = { pushHistory: true }) {
  if (!state.detail.active) {
    state.detail.tabBeforeOpen = state.tab;
    state.detail.stack = [];
  } else if (options.pushHistory && state.detail.current) {
    state.detail.stack.push(state.detail.current);
  }
  state.detail.active = true;
  state.detail.type = type;
  state.detail.current = { type, title, payload };

  el.main.classList.add("detail-open");
  el.detailView.classList.add("open");
  el.detailView.setAttribute("aria-hidden", "false");
  el.detailTitle.textContent = title;

  el.photoDetailPage.classList.toggle("hidden", type !== "photo");
  el.albumDetailPage.classList.toggle("hidden", type !== "album");
  el.photoDetailNav.classList.toggle("hidden", type !== "photo");
  if (type !== "photo") {
    togglePhotoHelp(false);
  }
  if (el.btnDetailBack) {
    el.btnDetailBack.disabled = state.detail.stack.length === 0;
  }
}

function hideDetailView() {
  state.detail.active = false;
  state.detail.type = null;
  state.detail.current = null;
  state.detail.stack = [];
  el.main.classList.remove("detail-open");
  el.detailView.classList.remove("open");
  el.detailView.setAttribute("aria-hidden", "true");
  togglePhotoHelp(false);
  if (el.btnDetailBack) {
    el.btnDetailBack.disabled = true;
  }
}

function goDetailBack() {
  if (state.detail.stack.length > 0) {
    const prev = state.detail.stack.pop();
    if (prev?.type === "album" && prev.payload?.albumId) {
      openAlbumDetailPage(prev.payload.albumId, { pushHistory: false });
      return;
    }
    if (prev?.type === "photo" && prev.payload?.photoId) {
      openPhotoDetailPage(prev.payload.photoId, { pushHistory: false });
      return;
    }
  }

  goDetailUp();
}

function goDetailUp() {
  hideDetailView();
}

function setTab(tab) {
  const valid = ["photos", "favorites", "albums", "settings", "status"];
  state.tab = valid.includes(tab) ? tab : "photos";
  if (state.detail.active) {
    goDetailUp();
  }
  location.hash = state.tab;
  updateTabUi();
  if (state.tab === "favorites") {
    void loadFavoritePhotos();
  }
}

function initTabFromHash() {
  const hashTab = location.hash.replace("#", "");
  state.tab = hashTab || "photos";
  updateTabUi();
}

function readPhotoStateFromUrl() {
  const params = new URLSearchParams(location.search);
  state.photoFilters.keyword = params.get("pk") || "";
  state.photoFilters.album_id = params.get("pa") || "";
  state.photoFilters.order = params.get("po") || "desc";
  state.photoFilters.start_time = params.get("ps") || "";
  state.photoFilters.end_time = params.get("pe") || "";
  state.photoPage = Math.max(1, Number(params.get("pp") || "1") || 1);

  const pageSize = Math.min(100, Math.max(1, Number(params.get("psz") || "36") || 36));
  el.photoKeyword.value = state.photoFilters.keyword;
  el.photoOrder.value = state.photoFilters.order;
  el.photoStartDate.value = isoToDateInput(state.photoFilters.start_time);
  el.photoEndDate.value = isoToDateInput(state.photoFilters.end_time);
  el.photoPageSize.value = String(pageSize);
}

function writePhotoStateToUrl() {
  const url = new URL(location.href);
  const params = url.searchParams;
  const setOrDelete = (key, value) => {
    if (value) {
      params.set(key, value);
    } else {
      params.delete(key);
    }
  };

  setOrDelete("pk", state.photoFilters.keyword);
  setOrDelete("pa", state.photoFilters.album_id);
  setOrDelete("po", state.photoFilters.order);
  setOrDelete("ps", state.photoFilters.start_time);
  setOrDelete("pe", state.photoFilters.end_time);
  setOrDelete("pp", String(state.photoPage || 1));
  setOrDelete("psz", String(Number(el.photoPageSize.value) || 36));
  history.replaceState(null, "", url);
}

async function checkHealth() {
  setApiBase(el.apiBase.value.trim());
  setHealthText("检查中...");
  try {
    const data = await api("/health", { method: "GET" });
    setHealthText(`服务在线: ${data.status}`);
  } catch (err) {
    setHealthText(`服务不可用: ${err.message}`, false);
  }
}

function renderSources(items) {
  el.sourceList.innerHTML = "";
  if (!items.length) {
    el.sourceList.innerHTML = '<p class="hint">暂无 source，请先创建。</p>';
    return;
  }

  for (const source of items) {
    const node = el.sourceItemTpl.content.firstElementChild.cloneNode(true);
    node.querySelector(".item-title").textContent = source.name;
    node.querySelector(".item-sub").textContent = source.root_path;
    const btn = node.querySelector("button");
    btn.addEventListener("click", async () => {
      btn.disabled = true;
      btn.textContent = "扫描中...";
      try {
        const data = await api(`/sources/${source.id}/scan`, { method: "POST", body: "{}" });
        el.sourceMessage.textContent = `已触发扫描: ${data.job_id.slice(0, 12)}...`;
        await Promise.all([loadTaskOverview(), loadTaskJobs()]);
      } catch (err) {
        el.sourceMessage.textContent = `触发失败: ${err.message}`;
      } finally {
        btn.disabled = false;
        btn.textContent = "触发扫描";
      }
    });
    el.sourceList.appendChild(node);
  }
}

async function loadSources() {
  try {
    const items = await api("/sources", { method: "GET" });
    const next = {};
    for (const src of items || []) {
      if (src?.id) next[src.id] = src;
    }
    state.sourceById = next;
    renderSources(items);
  } catch (err) {
    el.sourceList.innerHTML = `<p class="hint">加载失败: ${escapeHtml(err.message)}</p>`;
  }
}

async function createSource(event) {
  event.preventDefault();
  el.sourceMessage.textContent = "创建中...";
  try {
    await api("/sources", {
      method: "POST",
      body: JSON.stringify({
        name: el.sourceName.value.trim(),
        root_path: el.sourcePath.value.trim(),
        source_type: "local_fs",
      }),
    });
    el.sourceMessage.textContent = "创建成功";
    showToast("Source 创建成功");
    el.sourceForm.reset();
    await loadSources();
  } catch (err) {
    el.sourceMessage.textContent = `创建失败: ${err.message}`;
    showToast(`创建失败: ${err.message}`, "error");
  }
}

function bindPhotoThumbFallback(container, imgAttr, fallbackAttr) {
  container.querySelectorAll(`[${imgAttr}]`).forEach((img) => {
    img.addEventListener("error", () => {
      img.classList.add("hidden");
      const id = img.getAttribute(imgAttr);
      if (!id) return;
      const fallback = container.querySelector(`[${fallbackAttr}="${id}"]`);
      if (fallback) {
        fallback.classList.remove("hidden");
      }
    });
  });
}

function bindHorizontalDragScroll(container) {
  let down = false;
  let startX = 0;
  let startScroll = 0;

  container.addEventListener("mousedown", (event) => {
    down = true;
    container.classList.add("dragging");
    startX = event.clientX;
    startScroll = container.scrollLeft;
  });

  container.addEventListener("mouseleave", () => {
    down = false;
    container.classList.remove("dragging");
  });

  container.addEventListener("mouseup", () => {
    down = false;
    container.classList.remove("dragging");
  });

  container.addEventListener("mousemove", (event) => {
    if (!down) return;
    const delta = event.clientX - startX;
    container.scrollLeft = startScroll - delta;
  });

  container.addEventListener(
    "wheel",
    (event) => {
      if (Math.abs(event.deltaY) <= Math.abs(event.deltaX)) return;
      event.preventDefault();
      container.scrollLeft += event.deltaY;
    },
    { passive: false },
  );
}

const photoView = createPhotoView({
  state,
  el,
  api,
  escapeHtml,
  showToast,
  writePhotoStateToUrl,
  saveSearchHistory,
  closeSearchSuggestions,
  dateToStartIso,
  dateToEndIso,
  formatPhotoGroupLabel,
  formatShotTime,
  buildMapUrl,
  photoVisualStyle,
  bindPhotoThumbFallback,
  openDetailView,
  goDetailUp,
});

const {
  addPhotoToAlbum,
  applyPhotoTransform,
  endPhotoDrag,
  fetchAndRenderPhotos,
  isDrawerOpen,
  loadFavoritePage,
  loadFavoritePhotos,
  loadPhotoPage,
  moveDrawerPhoto,
  movePhotoDrag,
  onPhotoDoubleClick,
  onPhotoWheel,
  openPhotoDetailPage,
  resetPhotoView,
  saveRemark,
  searchPhotos,
  startPhotoDrag,
  syncPhotoFiltersFromForm,
  togglePhotoHelp,
  toggleSelectedPhotoFavorite,
  zoomPhoto,
} = photoView;

const albumView = createAlbumView({
  state,
  el,
  api,
  escapeHtml,
  showToast,
  albumVisualStyle,
  photoVisualStyle,
  formatPhotoGroupLabel,
  formatShotTime,
  formatIsoToSecond,
  dateToStartIso,
  dateToEndIso,
  isoToDateInput,
  bindPhotoThumbFallback,
  openDetailView,
  getOpenPhotoDetailPage: () => openPhotoDetailPage,
});

const { loadAlbumDetailPage, loadAlbums, openAlbumDetailPage, searchAlbumDetailPhotos } = albumView;

const taskView = createTaskView({
  state,
  el,
  api,
  escapeHtml,
});

const {
  cancelScan,
  cancelTask,
  loadTaskDetail,
  loadTaskJobs,
  loadTaskOverview,
  retryScan,
  retryTask,
  startTaskAutoRefresh,
} = taskView;

function isTypingTarget(target) {
  return target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement || target instanceof HTMLSelectElement;
}

function onKeydown(event) {
  if (event.key === "Escape" && state.detail.active) {
    goDetailBack();
    return;
  }

  if (state.detail.active && state.detail.type === "photo") {
    if (event.key === "?" || (event.shiftKey && event.key === "/")) {
      event.preventDefault();
      togglePhotoHelp();
      return;
    }
    if (event.key === "ArrowLeft") {
      event.preventDefault();
      moveDrawerPhoto(-1);
      return;
    }
    if (event.key === "ArrowRight") {
      event.preventDefault();
      moveDrawerPhoto(1);
      return;
    }
    if (event.key === "+" || event.key === "=") {
      event.preventDefault();
      zoomPhoto(0.2);
      return;
    }
    if (event.key === "-") {
      event.preventDefault();
      zoomPhoto(-0.2);
      return;
    }
    if (event.key === "0") {
      event.preventDefault();
      resetPhotoView();
      return;
    }
  }

  if (state.tab !== "photos") {
    return;
  }

  if (isTypingTarget(event.target)) {
    return;
  }

  if (event.key === "/") {
    event.preventDefault();
    el.photoKeyword.focus();
    return;
  }

  if (event.key === "ArrowLeft") {
    event.preventDefault();
    if (isDrawerOpen()) {
      moveDrawerPhoto(-1);
    } else {
      loadPhotoPage(state.photoPage - 1);
    }
    return;
  }

  if (event.key === "ArrowRight") {
    event.preventDefault();
    if (isDrawerOpen()) {
      moveDrawerPhoto(1);
    } else {
      loadPhotoPage(state.photoPage + 1);
    }
  }
}

function bindEvents() {
  const sidebarCollapsed = localStorage.getItem("xphoto_sidebar_collapsed") === "1";
  setSidebarCollapsed(sidebarCollapsed);
  if (el.btnDetailBack) {
    el.btnDetailBack.disabled = true;
  }

  el.apiBase.value = state.apiBase;
  el.apiBase.addEventListener("change", () => setApiBase(el.apiBase.value.trim()));
  el.btnHealth.addEventListener("click", checkHealth);
  el.favoritePageSize.value = String(state.favorite.pageSize);

  window.addEventListener("hashchange", () => {
    const hashTab = location.hash.replace("#", "") || "photos";
    setTab(hashTab);
  });

  el.sideNav.addEventListener("click", (event) => {
    const target = event.target;
    if (!(target instanceof HTMLElement)) return;
    const btn = target.closest("button[data-tab]");
    if (!(btn instanceof HTMLButtonElement)) return;
    const tab = btn.dataset.tab;
    if (tab) {
      setTab(tab);
    }
  });
  el.btnSidebarToggle.addEventListener("click", toggleSidebarCollapsed);

  el.btnQuickSearch.addEventListener("click", searchPhotos);
  el.photoKeyword.addEventListener("keydown", (event) => {
    const suggestOpen = !el.searchSuggest.classList.contains("hidden") && state.search.items.length > 0;
    if (suggestOpen && event.key === "ArrowDown") {
      event.preventDefault();
      state.search.activeIndex = (state.search.activeIndex + 1) % state.search.items.length;
      renderSearchSuggestions();
      return;
    }
    if (suggestOpen && event.key === "ArrowUp") {
      event.preventDefault();
      state.search.activeIndex = (state.search.activeIndex - 1 + state.search.items.length) % state.search.items.length;
      renderSearchSuggestions();
      return;
    }
    if (event.key === "Enter") {
      if (suggestOpen && state.search.activeIndex >= 0) {
        event.preventDefault();
        const item = state.search.items[state.search.activeIndex];
        if (item) {
          applySearchSuggestion(item.value);
        }
        return;
      }
      searchPhotos(event);
      return;
    }
    if (event.key === "Escape") {
      closeSearchSuggestions();
    }
  });
  el.photoKeyword.addEventListener("input", () => {
    updateSearchExample();
    renderSearchSuggestions();
  });
  el.photoKeyword.addEventListener("focus", renderSearchSuggestions);
  el.photoKeyword.addEventListener("blur", () => {
    window.setTimeout(() => closeSearchSuggestions(), 120);
  });
  el.searchPrefixBar.querySelectorAll("[data-prefix]").forEach((btn) => {
    btn.addEventListener("click", () => {
      const prefix = btn.getAttribute("data-prefix");
      if (prefix) insertSearchPrefix(prefix);
    });
  });
  el.photoAlbumFilter.addEventListener("change", searchPhotos);
  el.photoOrder.addEventListener("change", searchPhotos);
  el.photoStartDate.addEventListener("change", searchPhotos);
  el.photoEndDate.addEventListener("change", searchPhotos);
  el.photoPageSize.addEventListener("change", () => {
    searchPhotos();
  });
  el.btnPhotoPrev.addEventListener("click", () => loadPhotoPage(state.photoPage - 1));
  el.btnPhotoNext.addEventListener("click", () => loadPhotoPage(state.photoPage + 1));

  el.btnReloadAlbumsPhotos.addEventListener("click", loadAlbums);
  el.btnReloadAlbums.addEventListener("click", loadAlbums);
  el.favoritePageSize.addEventListener("change", () => {
    state.favorite.page = 1;
    void loadFavoritePhotos();
  });
  el.btnReloadFavorites.addEventListener("click", loadFavoritePhotos);
  el.btnFavoritePrev.addEventListener("click", () => loadFavoritePage(state.favorite.page - 1));
  el.btnFavoriteNext.addEventListener("click", () => loadFavoritePage(state.favorite.page + 1));

  el.sourceForm.addEventListener("submit", createSource);
  el.btnReloadSources.addEventListener("click", loadSources);

  el.btnTaskOverview.addEventListener("click", loadTaskOverview);
  el.btnTaskList.addEventListener("click", loadTaskJobs);
  el.taskFilterForm.addEventListener("submit", loadTaskJobs);
  el.btnCancelTask.addEventListener("click", cancelTask);
  el.btnRetryTask.addEventListener("click", retryTask);
  el.btnCancelScan.addEventListener("click", cancelScan);
  el.btnRetryScan.addEventListener("click", retryScan);

  el.btnDetailBack.addEventListener("click", goDetailBack);
  el.btnDetailUp.addEventListener("click", goDetailUp);
  el.btnDrawerPrev.addEventListener("click", () => moveDrawerPhoto(-1));
  el.btnDrawerNext.addEventListener("click", () => moveDrawerPhoto(1));
  el.btnSaveRemark.addEventListener("click", saveRemark);
  el.btnAddToAlbum.addEventListener("click", addPhotoToAlbum);
  el.btnPhotoZoomIn.addEventListener("click", () => zoomPhoto(0.2));
  el.btnPhotoZoomOut.addEventListener("click", () => zoomPhoto(-0.2));
  el.btnPhotoZoomReset.addEventListener("click", resetPhotoView);
  el.btnPhotoFavorite.addEventListener("click", toggleSelectedPhotoFavorite);
  el.btnPhotoHelp.addEventListener("click", () => togglePhotoHelp());

  el.photoPreviewImg.addEventListener("wheel", onPhotoWheel);
  el.photoPreviewImg.addEventListener("mousedown", startPhotoDrag);
  el.photoPreviewImg.addEventListener("dblclick", onPhotoDoubleClick);
  el.photoPreviewImg.addEventListener("load", () => {
    applyPhotoTransform();
  });
  el.photoPreviewImg.addEventListener("error", () => {
    el.photoDetailMsg.textContent = "照片加载失败：若文件为 HEIC/HEIF，可能是浏览器不支持该格式。";
    showToast("照片加载失败：可能是浏览器不支持 HEIC/HEIF", "error");
  });
  window.addEventListener("mouseup", endPhotoDrag);
  window.addEventListener("mousemove", movePhotoDrag);

  el.btnAlbumDetailSearch.addEventListener("click", searchAlbumDetailPhotos);
  el.albumDetailKeyword.addEventListener("keydown", (event) => {
    if (event.key === "Enter") {
      searchAlbumDetailPhotos();
    }
  });
  el.albumDetailOrder.addEventListener("change", searchAlbumDetailPhotos);
  el.albumDetailStartDate.addEventListener("change", searchAlbumDetailPhotos);
  el.albumDetailEndDate.addEventListener("change", searchAlbumDetailPhotos);
  el.albumDetailPageSize.addEventListener("change", searchAlbumDetailPhotos);
  el.btnAlbumDetailPrev.addEventListener("click", () => loadAlbumDetailPage(state.albumDetail.page - 1));
  el.btnAlbumDetailNext.addEventListener("click", () => loadAlbumDetailPage(state.albumDetail.page + 1));

  document.addEventListener("keydown", onKeydown);
  document.addEventListener("click", (event) => {
    const target = event.target;
    if (!(target instanceof HTMLElement)) return;
    if (target.closest(".search-wrap")) return;
    closeSearchSuggestions();
  });
  bindHorizontalDragScroll(el.latestAlbumsRow);
}

async function boot() {
  state.search.history = loadSearchHistory();
  readPhotoStateFromUrl();
  bindEvents();
  startTaskAutoRefresh();
  initTabFromHash();
  setTab(state.tab || "photos");
  updateSearchExample();
  await checkHealth();
  await Promise.all([loadAlbums(), loadSources(), loadTaskOverview(), loadTaskJobs()]);
  syncPhotoFiltersFromForm(false);
  await fetchAndRenderPhotos();
}

boot();
