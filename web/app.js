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

function getFavoritePageSize() {
  const size = Math.min(100, Math.max(1, Number(el.favoritePageSize.value) || state.favorite.pageSize || 36));
  state.favorite.pageSize = size;
  localStorage.setItem("xphoto_favorite_page_size", String(size));
  if (el.favoritePageSize.value !== String(size)) {
    el.favoritePageSize.value = String(size);
  }
  return size;
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

function togglePhotoHelp(force) {
  if (!el.photoHelpPanel) return;
  const willShow = typeof force === "boolean" ? force : el.photoHelpPanel.classList.contains("hidden");
  el.photoHelpPanel.classList.toggle("hidden", !willShow);
  el.photoHelpPanel.setAttribute("aria-hidden", willShow ? "false" : "true");
}

function parseExifJson(exifJson) {
  if (!exifJson) return null;
  if (typeof exifJson === "object") return exifJson;
  if (typeof exifJson !== "string") return null;
  try {
    return JSON.parse(exifJson);
  } catch (_err) {
    return null;
  }
}

function pickExif(exif, keys) {
  if (!exif || typeof exif !== "object") return null;
  for (const key of keys) {
    const v = exif[key];
    if (v !== undefined && v !== null && String(v).trim() !== "") {
      return String(v);
    }
  }
  return null;
}

function renderPhotoExif(p) {
  const exif = parseExifJson(p.exif_json);
  const rows = [
    ["相机", pickExif(exif, ["Model", "CameraModelName", "camera_model"])],
    ["镜头", pickExif(exif, ["LensModel", "Lens", "lens_model"])],
    ["光圈", pickExif(exif, ["FNumber", "ApertureValue", "aperture"])],
    ["快门", pickExif(exif, ["ExposureTime", "ShutterSpeedValue", "shutter"])],
    ["ISO", pickExif(exif, ["ISOSpeedRatings", "ISO", "iso"])],
    ["焦距", pickExif(exif, ["FocalLength", "focal_length"])],
    ["尺寸", p.width && p.height ? `${p.width} x ${p.height}` : null],
  ].filter(([, v]) => v);

  if (!rows.length) {
    el.photoExifMeta.innerHTML = "<p>状态</p><p>无可用 EXIF 参数</p>";
    return;
  }

  el.photoExifMeta.innerHTML = rows.map(([k, v]) => `<p>${escapeHtml(k)}</p><p>${escapeHtml(v)}</p>`).join("");
}

function clampPhotoPan() {
  const v = state.detail.photoView;
  const stage = el.photoPreviewImg.parentElement;
  if (!stage) return;

  const stageW = stage.clientWidth;
  const stageH = stage.clientHeight;
  const imgW = el.photoPreviewImg.offsetWidth;
  const imgH = el.photoPreviewImg.offsetHeight;

  const maxX = Math.max(0, (imgW * v.scale - stageW) / 2);
  const maxY = Math.max(0, (imgH * v.scale - stageH) / 2);

  v.tx = Math.max(-maxX, Math.min(maxX, v.tx));
  v.ty = Math.max(-maxY, Math.min(maxY, v.ty));
}

function applyPhotoTransform() {
  const v = state.detail.photoView;
  clampPhotoPan();
  el.photoPreviewImg.style.transform = `translate(${v.tx}px, ${v.ty}px) scale(${v.scale})`;
  if (el.photoZoomBadge) {
    el.photoZoomBadge.textContent = `${Math.round(v.scale * 100)}%`;
  }
}

function zoomPhoto(step) {
  zoomPhotoAtPoint(step, null, null);
}

function zoomPhotoAtPoint(step, clientX, clientY) {
  const v = state.detail.photoView;
  const currentScale = v.scale;
  const nextScale = Math.min(5, Math.max(1, Number((currentScale + step).toFixed(2))));
  if (nextScale === currentScale) return;

  if (clientX == null || clientY == null || currentScale <= 0) {
    v.scale = nextScale;
    if (v.scale === 1) {
      v.tx = 0;
      v.ty = 0;
    }
    applyPhotoTransform();
    return;
  }

  const rect = el.photoPreviewImg.getBoundingClientRect();
  const centerX = rect.left + rect.width / 2;
  const centerY = rect.top + rect.height / 2;

  const relativeX = (clientX - centerX - v.tx) / currentScale;
  const relativeY = (clientY - centerY - v.ty) / currentScale;

  v.scale = nextScale;
  if (v.scale === 1) {
    v.tx = 0;
    v.ty = 0;
  } else {
    v.tx = v.tx + relativeX * (currentScale - nextScale);
    v.ty = v.ty + relativeY * (currentScale - nextScale);
  }

  applyPhotoTransform();
}

function toggleZoomAtPoint(clientX, clientY) {
  const v = state.detail.photoView;
  const target = v.scale < 1.8 ? 2 : 1;
  const step = target - v.scale;
  if (Math.abs(step) < 0.001) return;
  zoomPhotoAtPoint(step, clientX, clientY);
}

function startPhotoDrag(event) {
  event.preventDefault();
  if (state.detail.photoView.scale <= 1) return;
  state.detail.photoView.dragging = true;
  state.detail.photoView.dragStartX = event.clientX - state.detail.photoView.tx;
  state.detail.photoView.dragStartY = event.clientY - state.detail.photoView.ty;
  el.photoPreviewImg.classList.add("dragging");
}

function endPhotoDrag() {
  state.detail.photoView.dragging = false;
  el.photoPreviewImg.classList.remove("dragging");
}

function movePhotoDrag(event) {
  if (!state.detail.photoView.dragging || state.detail.photoView.scale <= 1) return;
  state.detail.photoView.tx = event.clientX - state.detail.photoView.dragStartX;
  state.detail.photoView.ty = event.clientY - state.detail.photoView.dragStartY;
  applyPhotoTransform();
}

function onPhotoWheel(event) {
  if (!isDrawerOpen()) return;
  event.preventDefault();
  zoomPhotoAtPoint(event.deltaY < 0 ? 0.15 : -0.15, event.clientX, event.clientY);
}

function onPhotoDoubleClick(event) {
  if (!isDrawerOpen()) return;
  event.preventDefault();
  toggleZoomAtPoint(event.clientX, event.clientY);
}

function resetPhotoView() {
  state.detail.photoView.scale = 1;
  state.detail.photoView.tx = 0;
  state.detail.photoView.ty = 0;
  state.detail.photoView.dragging = false;
  el.photoPreviewImg.classList.remove("dragging");
  applyPhotoTransform();
}

async function api(path, options = {}) {
  const controller = new AbortController();
  const timeout = window.setTimeout(() => controller.abort(), 15000);
  try {
    const res = await fetch(`${state.apiBase}${path}`, {
      headers: { "content-type": "application/json", ...(options.headers || {}) },
      signal: controller.signal,
      ...options,
    });
    const data = await res.json();
    if (!res.ok || data.code !== 0) {
      throw new Error(data.message || `HTTP ${res.status}`);
    }
    return data.data;
  } catch (err) {
    if (err.name === "AbortError") {
      throw new Error("请求超时，请检查服务地址或网络连通性");
    }
    throw err;
  } finally {
    window.clearTimeout(timeout);
  }
}

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

function renderAlbumCards(target, albums) {
  if (!albums.length) {
    target.innerHTML = '<p class="hint">暂无相册。</p>';
    return;
  }
  target.innerHTML = albums
    .map(
      (a) => {
        const visual = albumVisualStyle(a);
        const title = a.name || "未命名相册";
        const count = state.albumPhotoCounts[a.id];
        const countLabel = Number.isFinite(count) ? `${count} 项` : "...";
        return `
      <article class="album-card" data-album-id="${a.id}">
        <div class="album-cover" style="background:${visual.background};">
        </div>
        <p class="album-name">${escapeHtml(title)}</p>
        <p class="album-count">${escapeHtml(countLabel)}</p>
      </article>
    `;
      },
    )
    .join("");

  target.querySelectorAll(".album-card[data-album-id]").forEach((item) => {
    item.addEventListener("click", () => {
      const albumId = item.getAttribute("data-album-id");
      if (albumId) {
        openAlbumDetailPage(albumId);
      }
    });
  });
}

async function loadAlbumPhotoCounts(albums) {
  const ids = albums.map((a) => a.id);
  const counts = await Promise.all(
    ids.map(async (albumId) => {
      try {
        const data = await api("/photos/search", {
          method: "POST",
          body: JSON.stringify({ album_id: albumId, page: 1, page_size: 1 }),
        });
        return [albumId, Number(data.total || 0)];
      } catch (_err) {
        return [albumId, null];
      }
    }),
  );

  for (const [albumId, count] of counts) {
    if (Number.isFinite(count)) {
      state.albumPhotoCounts[albumId] = count;
    }
  }
}

async function loadAlbums() {
  el.latestAlbumsRow.innerHTML = '<div class="loading">加载最新相册...</div>';
  if (state.tab === "albums") {
    el.albumGrid.innerHTML = '<div class="loading">加载相册列表...</div>';
  }
  try {
    state.albums = await api("/albums", { method: "GET" });
    renderAlbumCards(el.albumGrid, state.albums);
    renderAlbumCards(el.latestAlbumsRow, state.albums.slice(0, 8));

    loadAlbumPhotoCounts(state.albums)
      .then(() => {
        renderAlbumCards(el.albumGrid, state.albums);
        renderAlbumCards(el.latestAlbumsRow, state.albums.slice(0, 8));
      })
      .catch(() => {
        // ignore count refresh errors
      });

    el.photoAlbumSelect.innerHTML = state.albums.length
      ? state.albums.map((a) => `<option value="${a.id}">${escapeHtml(a.name)}</option>`).join("")
      : '<option value="">无可选相册</option>';

    el.photoAlbumFilter.innerHTML =
      '<option value="">全部相册</option>' +
      state.albums.map((a) => `<option value="${a.id}">${escapeHtml(a.name)}</option>`).join("");

    if (state.photoFilters.album_id) {
      el.photoAlbumFilter.value = state.photoFilters.album_id;
    }
  } catch (err) {
    el.albumGrid.innerHTML = `<p class="hint">加载失败: ${escapeHtml(err.message)}</p>`;
    el.latestAlbumsRow.innerHTML = `<p class="hint">加载失败: ${escapeHtml(err.message)}</p>`;
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

function renderPhotoFilterChips() {
  const chips = [];
  if (state.photoFilters.keyword) {
    chips.push(`关键词: ${escapeHtml(state.photoFilters.keyword)}`);
  }
  if (state.photoFilters.album_id) {
    const album = state.albums.find((a) => a.id === state.photoFilters.album_id);
    chips.push(`相册: ${escapeHtml(album ? album.name : state.photoFilters.album_id)}`);
  }
  chips.push(state.photoFilters.order === "asc" ? "排序: 时间正序" : "排序: 时间倒序");
  if (state.photoFilters.start_time) {
    chips.push(`开始: ${escapeHtml(state.photoFilters.start_time.slice(0, 10))}`);
  }
  if (state.photoFilters.end_time) {
    chips.push(`结束: ${escapeHtml(state.photoFilters.end_time.slice(0, 10))}`);
  }
  el.photoFilterChips.innerHTML = chips.map((txt) => `<span class="chip">${txt}</span>`).join("");
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

function renderPhotos(data) {
  const items = data.items || [];
  state.photoItems = items.map((p) => p.id);
  state.photoTotal = data.total ?? 0;
  state.photoPage = data.page ?? 1;

  const pageSize = Number(el.photoPageSize.value) || 36;
  const totalPages = Math.max(1, Math.ceil(state.photoTotal / pageSize));
  el.photoPageHint.textContent = `第 ${state.photoPage} / ${totalPages} 页 · 共 ${state.photoTotal} 张`;
  el.btnPhotoPrev.disabled = state.photoPage <= 1;
  el.btnPhotoNext.disabled = state.photoPage >= totalPages;
  renderPhotoFilterChips();
  writePhotoStateToUrl();

  if (!items.length) {
    el.photoMasonry.innerHTML = '<p class="hint">没有匹配结果。</p>';
    return;
  }

  const groups = new Map();
  for (const p of items) {
    const key = (p.sort_time || "").slice(0, 10) || "未知日期";
    if (!groups.has(key)) {
      groups.set(key, []);
    }
    groups.get(key).push(p);
  }

  const groupKeys = Array.from(groups.keys());
  el.photoMasonry.innerHTML = groupKeys
    .map((key) => {
      const photos = groups.get(key) || [];
      const cards = photos
        .map((p) => {
          const visual = photoVisualStyle(p);
          return `
        <article class="photo-card" data-photo-id="${p.id}">
          <div class="photo-thumb" style="height:${visual.height}px;background:${visual.background};">
            <img class="photo-thumb-img" data-photo-thumb-id="${p.id}" src="${state.apiBase}/photos/${p.id}/file" alt="${escapeHtml(p.file_name || "照片")}" loading="lazy" />
            <div class="photo-thumb-fallback hidden" data-photo-thumb-fallback-id="${p.id}">${escapeHtml((p.file_ext || "img").toUpperCase())}</div>
            <div class="photo-title">${escapeHtml(p.file_name)}</div>
          </div>
          <p class="item-sub">拍摄时间: ${escapeHtml(formatShotTime(p.sort_time))}</p>
        </article>
      `;
        })
        .join("");
      return `
      <section class="photo-group">
        <h3>${escapeHtml(formatPhotoGroupLabel(key))} <span class="hint">${escapeHtml(key)}</span></h3>
        <div class="masonry-grid">${cards}</div>
      </section>
    `;
    })
    .join("");

  el.photoMasonry.querySelectorAll("[data-photo-id]").forEach((card) => {
    card.addEventListener("click", () => {
      const photoId = card.getAttribute("data-photo-id");
      if (photoId) openPhotoDetailPage(photoId);
    });
  });

  bindPhotoThumbFallback(el.photoMasonry, "data-photo-thumb-id", "data-photo-thumb-fallback-id");
}

function renderFavoritePhotos(data) {
  const items = (data.items || []).map((item) => {
    if (item && item.photo) {
      return { ...item.photo, favorite_at: item.favorite_at || "" };
    }
    return item;
  });
  state.photoItems = items.map((p) => p.id);
  state.favorite.total = data.total ?? 0;
  state.favorite.page = data.page ?? 1;
  const pageSize = getFavoritePageSize();
  const totalPages = Math.max(1, Math.ceil(state.favorite.total / pageSize));
  el.favoritePageHint.textContent = `第 ${state.favorite.page} / ${totalPages} 页 · 共 ${state.favorite.total} 张`;
  el.btnFavoritePrev.disabled = state.favorite.page <= 1;
  el.btnFavoriteNext.disabled = state.favorite.page >= totalPages;

  if (!items.length) {
    el.favoriteMasonry.innerHTML = '<p class="hint">暂无收藏照片。</p>';
    return;
  }

  const groups = new Map();
  for (const p of items) {
    const key = (p.sort_time || "").slice(0, 10) || "未知日期";
    if (!groups.has(key)) {
      groups.set(key, []);
    }
    groups.get(key).push(p);
  }

  const groupKeys = Array.from(groups.keys());
  el.favoriteMasonry.innerHTML = groupKeys
    .map((key) => {
      const photos = groups.get(key) || [];
      const cards = photos
        .map((p) => {
          const visual = photoVisualStyle(p);
          return `
        <article class="photo-card" data-favorite-photo-id="${p.id}">
          <button class="favorite-remove-btn" type="button" data-favorite-remove-id="${p.id}" title="取消收藏">取消收藏</button>
          <div class="photo-thumb" style="height:${visual.height}px;background:${visual.background};">
            <img class="photo-thumb-img" data-favorite-thumb-id="${p.id}" src="${state.apiBase}/photos/${p.id}/file" alt="${escapeHtml(p.file_name || "照片")}" loading="lazy" />
            <div class="photo-thumb-fallback hidden" data-favorite-thumb-fallback-id="${p.id}">${escapeHtml((p.file_ext || "img").toUpperCase())}</div>
            <div class="photo-title">${escapeHtml(p.file_name)}</div>
          </div>
          <p class="item-sub">拍摄时间: ${escapeHtml(formatShotTime(p.sort_time))}</p>
          <p class="item-sub">收藏时间: ${escapeHtml(formatShotTime(p.favorite_at))}</p>
        </article>
      `;
        })
        .join("");
      return `
      <section class="photo-group">
        <h3>${escapeHtml(formatPhotoGroupLabel(key))} <span class="hint">${escapeHtml(key)}</span></h3>
        <div class="masonry-grid">${cards}</div>
      </section>
    `;
    })
    .join("");

  el.favoriteMasonry.querySelectorAll("[data-favorite-photo-id]").forEach((card) => {
    card.addEventListener("click", () => {
      const photoId = card.getAttribute("data-favorite-photo-id");
      if (photoId) openPhotoDetailPage(photoId);
    });
  });

  bindPhotoThumbFallback(el.favoriteMasonry, "data-favorite-thumb-id", "data-favorite-thumb-fallback-id");

  el.favoriteMasonry.querySelectorAll("[data-favorite-remove-id]").forEach((btn) => {
    btn.addEventListener("click", async (event) => {
      event.stopPropagation();
      const photoId = btn.getAttribute("data-favorite-remove-id");
      if (!photoId) return;
      btn.disabled = true;
      try {
        await api(`/photos/${photoId}/favorite`, {
          method: "PATCH",
          body: JSON.stringify({ favorite: false }),
        });
        showToast("已取消收藏");
        await loadFavoritePhotos();
        if (state.detail.active && state.detail.type === "photo" && state.selectedPhotoId === photoId) {
          if (state.photoItems.length > 0) {
            await openPhotoDetailPage(state.photoItems[0], { pushHistory: false });
          } else {
            goDetailUp();
          }
        }
      } catch (err) {
        showToast(`取消收藏失败: ${err.message}`, "error");
      } finally {
        btn.disabled = false;
      }
    });
  });
}

async function loadFavoritePhotos() {
  el.favoriteMasonry.innerHTML = '<div class="loading">加载收藏照片中...</div>';
  try {
    const pageSize = getFavoritePageSize();
    const data = await api(`/photos/favorites?page=${state.favorite.page}&page_size=${pageSize}`, {
      method: "GET",
    });
    renderFavoritePhotos(data);
  } catch (err) {
    el.favoriteMasonry.innerHTML = `<p class="hint">加载失败: ${escapeHtml(err.message)}</p>`;
  }
}

async function loadFavoritePage(page) {
  const pageSize = getFavoritePageSize();
  const totalPages = Math.max(1, Math.ceil(state.favorite.total / pageSize));
  state.favorite.page = Math.max(1, Math.min(totalPages, page));
  await loadFavoritePhotos();
}

function setPhotoFavoriteUi(isFavorite) {
  if (!el.btnPhotoFavorite) return;
  el.btnPhotoFavorite.classList.toggle("active", Boolean(isFavorite));
  el.btnPhotoFavorite.title = isFavorite ? "取消收藏" : "收藏";
}

async function toggleSelectedPhotoFavorite() {
  if (!state.selectedPhotoId) return;
  const currentPhotoId = state.selectedPhotoId;
  const next = !el.btnPhotoFavorite.classList.contains("active");
  el.btnPhotoFavorite.disabled = true;
  try {
    await api(`/photos/${currentPhotoId}/favorite`, {
      method: "PATCH",
      body: JSON.stringify({ favorite: next }),
    });
    setPhotoFavoriteUi(next);
    showToast(next ? "已收藏" : "已取消收藏");
    if (state.tab === "favorites") {
      await loadFavoritePhotos();
      if (!next && state.detail.active && state.detail.type === "photo" && !state.photoItems.includes(currentPhotoId)) {
        if (state.photoItems.length > 0) {
          await openPhotoDetailPage(state.photoItems[0], { pushHistory: false });
        } else {
          goDetailUp();
        }
      }
    }
  } catch (err) {
    showToast(`收藏操作失败: ${err.message}`, "error");
  } finally {
    el.btnPhotoFavorite.disabled = false;
  }
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

function syncPhotoFiltersFromForm(resetPage = false) {
  state.photoFilters.keyword = el.photoKeyword.value.trim();
  const hasAlbumPrefix = /(?:^|\s)album\s*:/i.test(state.photoFilters.keyword);
  if (hasAlbumPrefix) {
    state.photoFilters.album_id = "";
    el.photoAlbumFilter.value = "";
  } else {
    state.photoFilters.album_id = el.photoAlbumFilter.value;
  }
  state.photoFilters.order = el.photoOrder.value || "desc";
  state.photoFilters.start_time = dateToStartIso(el.photoStartDate.value);
  state.photoFilters.end_time = dateToEndIso(el.photoEndDate.value);
  if (resetPage) {
    state.photoPage = 1;
  }
}

async function fetchAndRenderPhotos() {
  el.photoMasonry.innerHTML = '<div class="loading">加载照片中...</div>';
  try {
    const data = await api("/photos/search", {
      method: "POST",
      body: JSON.stringify({
        keyword: state.photoFilters.keyword || undefined,
        album_id: state.photoFilters.album_id || undefined,
        order: state.photoFilters.order || "desc",
        start_time: state.photoFilters.start_time || undefined,
        end_time: state.photoFilters.end_time || undefined,
        page: state.photoPage,
        page_size: Number(el.photoPageSize.value) || 36,
      }),
    });
    renderPhotos(data);
  } catch (err) {
    el.photoMasonry.innerHTML = `<p class="hint">检索失败: ${escapeHtml(err.message)}</p>`;
    state.photoItems = [];
  }
}

async function searchPhotos(event) {
  if (event) event.preventDefault();
  syncPhotoFiltersFromForm(true);
  saveSearchHistory(state.photoFilters.keyword);
  closeSearchSuggestions();
  await fetchAndRenderPhotos();
}

async function loadPhotoPage(page) {
  const pageSize = Number(el.photoPageSize.value) || 36;
  const totalPages = Math.max(1, Math.ceil(state.photoTotal / pageSize));
  state.photoPage = Math.max(1, Math.min(totalPages, page));
  await fetchAndRenderPhotos();
}

function taskTag(status) {
  const warn = status === "failed" || status === "cancelled";
  return `<span class="tag ${warn ? "warn" : ""}">${escapeHtml(status)}</span>`;
}

function parseTaskCheckpoint(raw) {
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw);
    return parsed && typeof parsed === "object" ? parsed : null;
  } catch (_err) {
    return null;
  }
}

function parseTaskPayload(raw) {
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw);
    return parsed && typeof parsed === "object" ? parsed : null;
  } catch (_err) {
    return null;
  }
}

function getTaskSourceId(task, scanDetail) {
  if (scanDetail?.source_id) return String(scanDetail.source_id);
  const payload = parseTaskPayload(task?.payload_json);
  if (payload?.source_id) return String(payload.source_id);
  return "";
}

async function ensureSourceCache() {
  if (Object.keys(state.sourceById).length > 0) return;
  try {
    const items = await api("/sources", { method: "GET" });
    const next = {};
    for (const src of items || []) {
      if (src?.id) next[src.id] = src;
    }
    state.sourceById = next;
  } catch (_err) {
    // ignore source cache preload failures in status page
  }
}

function formatTaskProgress(task) {
  const done = Number(task.progress_done || 0);
  const total = task.progress_total;
  if (Number.isFinite(total) && total > 0) {
    const percent = Math.min(100, Math.round((done * 1000) / total) / 10);
    return `${done}/${total} (${percent}%)`;
  }
  return `${done}/${total ?? "-"}`;
}

function taskStageText(task) {
  if (task.status !== "running") return "-";
  if (task.error_message) return task.error_message;
  const checkpoint = parseTaskCheckpoint(task.checkpoint_json);
  if (checkpoint?.scan_status === "pending") return "等待扫描任务启动";
  return "扫描进行中";
}

function taskSourceBrief(task) {
  const sourceId = getTaskSourceId(task, null);
  if (!sourceId) return "source: -";
  const source = state.sourceById[sourceId];
  if (!source) return `source: ${sourceId.slice(0, 10)}...`;
  return `source: ${source.name}`;
}

function renderTaskOverview(data) {
  const active = data.active_tasks || [];
  const daemon = data.health?.daemon_tasks || [];
  if (!active.length && !daemon.length) {
    el.taskOverview.innerHTML = '<p class="hint">暂无活跃任务。</p>';
    return;
  }

  const activeHtml = active
    .map(
      (task) => `
      <div class="list-item">
        <div>
          <p class="item-title">${escapeHtml(task.job_type)} · ${escapeHtml(task.trigger_type)}</p>
          <p class="item-sub">${task.id.slice(0, 14)}... · ${escapeHtml(formatTaskProgress(task))}</p>
          <p class="item-sub">${escapeHtml(taskSourceBrief(task))}</p>
          <p class="item-sub">${escapeHtml(taskStageText(task))}</p>
        </div>
        ${taskTag(task.status)}
      </div>
    `,
    )
    .join("");

  const daemonHtml = daemon
    .map(
      (d) => `
      <div class="list-item">
        <div>
          <p class="item-title">daemon · ${escapeHtml(d.task_type)}</p>
          <p class="item-sub">${d.id.slice(0, 14)}... · heartbeat: ${escapeHtml(d.heartbeat_at || "-")}</p>
        </div>
        <span class="tag ${d.healthy ? "" : "warn"}">${d.healthy ? "healthy" : "stale"}</span>
      </div>
    `,
    )
    .join("");

  el.taskOverview.innerHTML = activeHtml + daemonHtml;
}

async function loadTaskOverview() {
  try {
    await ensureSourceCache();
    const data = await api("/task-jobs/overview", { method: "GET" });
    renderTaskOverview(data);
  } catch (err) {
    el.taskOverview.innerHTML = `<p class="hint">加载失败: ${escapeHtml(err.message)}</p>`;
  }
}

function renderTaskList(items) {
  if (!items.length) {
    el.taskList.innerHTML = '<p class="hint">当前条件下无任务。</p>';
    return;
  }

  el.taskList.innerHTML = items
    .map(
      (task) => `
      <article class="list-item clickable ${state.selectedTaskId === task.id ? "active" : ""}" data-task-id="${task.id}">
        <div>
          <p class="item-title">${escapeHtml(task.job_type)} · ${escapeHtml(task.trigger_type)}</p>
          <p class="item-sub">${task.id.slice(0, 14)}... · ${task.started_at || "未启动"}</p>
          <p class="item-sub">${escapeHtml(taskSourceBrief(task))}</p>
        </div>
        ${taskTag(task.status)}
      </article>
    `,
    )
    .join("");

  el.taskList.querySelectorAll("[data-task-id]").forEach((item) => {
    item.addEventListener("click", () => {
      const taskId = item.getAttribute("data-task-id");
      if (taskId) {
        state.selectedTaskId = taskId;
        loadTaskDetail(taskId);
      }
    });
  });
}

async function loadTaskJobs(event) {
  if (event) event.preventDefault();
  try {
    await ensureSourceCache();
    const jobType = el.taskJobType.value.trim();
    const triggerType = el.taskTriggerType.value.trim();
    const status = el.taskStatus.value.trim();
    state.taskJobTypeFilter = jobType;
    state.taskTriggerTypeFilter = triggerType;
    state.taskStatusFilter = status;

    const query = new URLSearchParams({ page: "1", page_size: "50" });
    if (jobType) query.set("job_type", jobType);
    if (status) query.set("status", status);

    const data = await api(`/task-jobs?${query.toString()}`, { method: "GET" });
    let items = data.items || [];
    if (triggerType) {
      items = items.filter((it) => String(it.trigger_type || "").toLowerCase() === triggerType.toLowerCase());
    }
    renderTaskList(items);
  } catch (err) {
    el.taskList.innerHTML = `<p class="hint">加载失败: ${escapeHtml(err.message)}</p>`;
  }
}

function renderTaskFlow(status) {
  const steps = ["pending", "running", "success"];
  if (status === "failed") steps[2] = "failed";
  if (status === "cancelled") steps[2] = "cancelled";
  const currentIdx = steps.indexOf(status);
  el.taskFlow.innerHTML = steps
    .map((step, idx) => {
      const cls = idx < currentIdx ? "done" : idx === currentIdx ? "current" : "pending";
      return `<li class="${cls}">${escapeHtml(step)}</li>`;
    })
    .join("");
}

function setTaskActionButtons(task, scanJobStatus) {
  const canCancelTask = task && (task.status === "pending" || task.status === "running");
  const canRetryTask = task && (task.status === "failed" || task.status === "cancelled");
  const canCancelScan = task?.scan_job_id && (scanJobStatus === "pending" || scanJobStatus === "running");
  const canRetryScan = task?.scan_job_id && (scanJobStatus === "failed" || scanJobStatus === "cancelled");

  el.btnCancelTask.disabled = !canCancelTask;
  el.btnRetryTask.disabled = !canRetryTask;
  el.btnCancelScan.disabled = !canCancelScan;
  el.btnRetryScan.disabled = !canRetryScan;
}

async function loadTaskDetail(taskId) {
  try {
    await ensureSourceCache();
    const task = await api(`/task-jobs/${taskId}`, { method: "GET" });
    state.selectedTask = task;
    state.selectedTaskId = task.id;
    let scanDetail = null;
    if (task.scan_job_id) {
      try {
        scanDetail = await api(`/scan-jobs/${task.scan_job_id}`, { method: "GET" });
      } catch (_err) {
        scanDetail = null;
      }
    }

    const scanStatus = scanDetail?.status || "-";
    const sourceId = getTaskSourceId(task, scanDetail) || "-";
    const source = state.sourceById[sourceId] || null;
    const sourceName = source?.name || "-";
    const sourcePath = source?.root_path || "-";
    state.selectedScanJobId = task.scan_job_id || null;
    el.taskDetailHint.textContent = `任务 ${task.id.slice(0, 12)}...`;
    renderTaskFlow(task.status);

    el.taskDetail.innerHTML = [
      ["task_id", task.id],
      ["status", task.status],
      ["trigger_type", task.trigger_type],
      ["source_id", sourceId],
      ["source_name", sourceName],
      ["source_path", sourcePath],
      ["scan_job_id", task.scan_job_id || "-"],
      ["scan_status", scanStatus],
      ["progress", formatTaskProgress(task)],
      ["stage", taskStageText(task)],
      ["error", task.status === "failed" || task.status === "cancelled" ? task.error_message || scanDetail?.error_message || "-" : "-"],
      ["started_at", task.started_at || "-"],
      ["finished_at", task.finished_at || scanDetail?.finished_at || "-"],
    ]
      .map(([k, v]) => `<p>${escapeHtml(k)}</p><p>${escapeHtml(v)}</p>`)
      .join("");

    setTaskActionButtons(task, scanStatus);
  } catch (err) {
    el.taskDetailHint.textContent = `加载失败: ${err.message}`;
  }
}

async function cancelTask() {
  if (!state.selectedTaskId) return;
  try {
    await api(`/task-jobs/${state.selectedTaskId}/cancel`, { method: "POST", body: "{}" });
    await Promise.all([loadTaskOverview(), loadTaskJobs(), loadTaskDetail(state.selectedTaskId)]);
  } catch (err) {
    el.taskDetailHint.textContent = `取消失败: ${err.message}`;
  }
}

async function retryTask() {
  if (!state.selectedTaskId) return;
  try {
    await api(`/task-jobs/${state.selectedTaskId}/retry`, { method: "POST", body: "{}" });
    await Promise.all([loadTaskOverview(), loadTaskJobs(), loadTaskDetail(state.selectedTaskId)]);
  } catch (err) {
    el.taskDetailHint.textContent = `重试失败: ${err.message}`;
  }
}

async function cancelScan() {
  if (!state.selectedScanJobId) return;
  try {
    await api(`/scan-jobs/${state.selectedScanJobId}/cancel`, { method: "POST", body: "{}" });
    if (state.selectedTaskId) {
      await Promise.all([loadTaskOverview(), loadTaskJobs(), loadTaskDetail(state.selectedTaskId)]);
    }
  } catch (err) {
    el.taskDetailHint.textContent = `取消扫描失败: ${err.message}`;
  }
}

async function retryScan() {
  if (!state.selectedScanJobId) return;
  try {
    const data = await api(`/scan-jobs/${state.selectedScanJobId}/retry`, { method: "POST", body: "{}" });
    state.selectedScanJobId = data.job_id;
    if (state.selectedTaskId) {
      await Promise.all([loadTaskOverview(), loadTaskJobs(), loadTaskDetail(state.selectedTaskId)]);
    }
  } catch (err) {
    el.taskDetailHint.textContent = `重试扫描失败: ${err.message}`;
  }
}

function isDrawerOpen() {
  return state.detail.active && state.detail.type === "photo";
}

function updateDrawerNavButtons() {
  const idx = state.photoItems.indexOf(state.selectedPhotoId || "");
  el.btnDrawerPrev.disabled = idx <= 0;
  el.btnDrawerNext.disabled = idx < 0 || idx >= state.photoItems.length - 1;
}

function moveDrawerPhoto(step) {
  const idx = state.photoItems.indexOf(state.selectedPhotoId || "");
  if (idx < 0) return;
  const target = idx + step;
  if (target < 0 || target >= state.photoItems.length) return;
  const nextPhotoId = state.photoItems[target];
  if (nextPhotoId) {
    openPhotoDetailPage(nextPhotoId);
  }
}

async function removePhotoFromAlbum(albumId) {
  if (!state.selectedPhotoId) return;
  el.photoDetailMsg.textContent = "移除中...";
  try {
    await api(`/albums/${albumId}/photos:remove`, {
      method: "POST",
      body: JSON.stringify({ photo_ids: [state.selectedPhotoId] }),
    });
    el.photoDetailMsg.textContent = "已从相册移除";
    showToast("已从相册移除");
    await openPhotoDetailPage(state.selectedPhotoId);
  } catch (err) {
    el.photoDetailMsg.textContent = `移除失败: ${err.message}`;
    showToast(`移除失败: ${err.message}`, "error");
  }
}

async function openPhotoDetailPage(photoId, options = { pushHistory: true }) {
  state.selectedPhotoId = photoId;
  el.photoDetailMsg.textContent = "加载详情...";
  setPhotoFavoriteUi(false);
  openDetailView("photo", "照片详情", { photoId }, options);
  togglePhotoHelp(false);
  updateDrawerNavButtons();
  try {
    const data = await api(`/photos/${photoId}`, { method: "GET" });
    const p = data.photo;
    el.detailTitle.textContent = `照片详情 · ${p.file_name || "未命名"}`;
    const v = Date.now();
    el.photoPreviewImg.src = `${state.apiBase}/photos/${photoId}/file?v=${v}`;
    el.photoPreviewImg.alt = p.file_name || "照片预览";
    resetPhotoView();
    renderPhotoExif(p);

    const lat = p.gps_lat;
    const lng = p.gps_lng;
    const hasGeo = Number.isFinite(lat) && Number.isFinite(lng);
    if (hasGeo) {
      el.photoGeoText.textContent = `${lat.toFixed(6)}, ${lng.toFixed(6)}`;
      el.photoGeoLink.href = buildMapUrl(lat, lng);
      el.photoGeoLink.classList.remove("hidden");
    } else {
      el.photoGeoText.textContent = "无定位信息";
      el.photoGeoLink.classList.add("hidden");
    }
    state.selectedPhotoAlbums = data.albums || [];
    const albumText = state.selectedPhotoAlbums.map((a) => a.name).join(" / ") || "-";
    el.photoDetailPrimary.innerHTML = [
      ["文件名", p.file_name],
      ["拍摄时间", formatShotTime(p.sort_time)],
      ["所属相册", albumText],
    ]
      .map(([k, v]) => `<p>${escapeHtml(k)}</p><p>${escapeHtml(v || "-")}</p>`)
      .join("");

    el.photoDetailAdvanced.innerHTML = [
      ["照片ID", p.id],
      ["文件路径", p.file_path],
      ["来源ID", p.source_id],
    ]
      .map(([k, v]) => `<p>${escapeHtml(k)}</p><p>${escapeHtml(v || "-")}</p>`)
      .join("");

    if (!state.selectedPhotoAlbums.length) {
      el.photoAlbums.innerHTML = '<span class="hint">未加入任何相册</span>';
    } else {
      el.photoAlbums.innerHTML = state.selectedPhotoAlbums
        .map(
          (a) =>
            `<span class="chip">${escapeHtml(a.name)}<button type="button" data-remove-album-id="${a.id}">移除</button></span>`,
        )
        .join("");
      el.photoAlbums.querySelectorAll("[data-remove-album-id]").forEach((btn) => {
        btn.addEventListener("click", () => {
          const albumId = btn.getAttribute("data-remove-album-id");
          if (albumId) removePhotoFromAlbum(albumId);
        });
      });
    }

    el.photoRemark.value = p.remark || "";
    setPhotoFavoriteUi(Boolean(data.is_favorite));
    const ext = String(p.file_ext || "").toLowerCase();
    const likelyUnsupported = ext === "heic" || ext === "heif";
    el.photoDetailMsg.textContent = likelyUnsupported ? "当前浏览器可能不支持 HEIC/HEIF 预览，请考虑转换为 JPG/PNG。" : "";
    updateDrawerNavButtons();
  } catch (err) {
    el.photoDetailMsg.textContent = `详情加载失败: ${err.message}`;
  }
}

function renderAlbumDetailPhotos(items) {
  if (!items.length) {
    el.albumDetailPhotos.innerHTML = '<p class="hint">该相册暂无照片。</p>';
    return;
  }

  const groups = new Map();
  for (const p of items) {
    const key = (p.sort_time || "").slice(0, 10) || "未知日期";
    if (!groups.has(key)) {
      groups.set(key, []);
    }
    groups.get(key).push(p);
  }

  const groupKeys = Array.from(groups.keys());
  el.albumDetailPhotos.innerHTML = groupKeys
    .map((key) => {
      const photos = groups.get(key) || [];
      const cards = photos
        .map((p) => {
          const visual = photoVisualStyle(p);
          return `
        <article class="photo-card" data-album-photo-id="${p.id}">
          <span class="preview-fab">预览</span>
          <div class="photo-thumb" style="height:${visual.height}px;background:${visual.background};">
            <img class="photo-thumb-img" data-album-thumb-id="${p.id}" src="${state.apiBase}/photos/${p.id}/file" alt="${escapeHtml(p.file_name || "照片")}" loading="lazy" />
            <div class="photo-thumb-fallback hidden" data-album-thumb-fallback-id="${p.id}">${escapeHtml((p.file_ext || "img").toUpperCase())}</div>
            <div class="photo-title">${escapeHtml(p.file_name)}</div>
          </div>
          <p class="item-sub">拍摄时间: ${escapeHtml(formatShotTime(p.sort_time))}</p>
          <span class="preview-entry">预览</span>
        </article>
      `;
        })
        .join("");
      return `
      <section class="photo-group">
        <h3>${escapeHtml(formatPhotoGroupLabel(key))} <span class="hint">${escapeHtml(key)}</span></h3>
        <div class="masonry-grid">${cards}</div>
      </section>
    `;
    })
    .join("");

  el.albumDetailPhotos.querySelectorAll("[data-album-photo-id]").forEach((node) => {
    node.addEventListener("click", () => {
      const photoId = node.getAttribute("data-album-photo-id");
      if (photoId) {
        openPhotoDetailPage(photoId);
      }
    });
  });

  bindPhotoThumbFallback(el.albumDetailPhotos, "data-album-thumb-id", "data-album-thumb-fallback-id");
}

function syncAlbumDetailFiltersFromForm(resetPage = false) {
  state.albumDetail.filters.keyword = el.albumDetailKeyword.value.trim();
  state.albumDetail.filters.order = el.albumDetailOrder.value || "desc";
  state.albumDetail.filters.start_time = dateToStartIso(el.albumDetailStartDate.value);
  state.albumDetail.filters.end_time = dateToEndIso(el.albumDetailEndDate.value);
  if (resetPage) {
    state.albumDetail.page = 1;
  }
}

function applyAlbumDetailFiltersToForm() {
  el.albumDetailKeyword.value = state.albumDetail.filters.keyword;
  el.albumDetailOrder.value = state.albumDetail.filters.order;
  el.albumDetailStartDate.value = isoToDateInput(state.albumDetail.filters.start_time);
  el.albumDetailEndDate.value = isoToDateInput(state.albumDetail.filters.end_time);
}

async function loadAlbumDetailMeta(albumId) {
  const data = await api(`/albums/${albumId}?page=1&page_size=1`, { method: "GET" });
  const album = data.album;
  state.albumDetail.albumName = album.name || "相册";

  el.detailTitle.textContent = `相册详情 · ${state.albumDetail.albumName}`;
  el.albumDetailMeta.innerHTML = [
    ["相册ID", album.id],
    ["相册名", album.name],
    ["创建方式", album.auto_created ? "自动" : "手动"],
    ["创建时间", formatIsoToSecond(album.created_at)],
    ["更新时间", formatIsoToSecond(album.updated_at)],
  ]
    .map(([k, v]) => `<p>${escapeHtml(k)}</p><p>${escapeHtml(v)}</p>`)
    .join("");
}

async function fetchAlbumDetailPhotos() {
  const albumId = state.albumDetail.albumId;
  if (!albumId) return;

  el.albumDetailPhotos.innerHTML = '<div class="loading">加载相册照片中...</div>';
  const pageSize = Number(el.albumDetailPageSize.value) || 48;

  const data = await api("/photos/search", {
    method: "POST",
    body: JSON.stringify({
      keyword: state.albumDetail.filters.keyword || undefined,
      album_id: albumId,
      order: state.albumDetail.filters.order,
      start_time: state.albumDetail.filters.start_time || undefined,
      end_time: state.albumDetail.filters.end_time || undefined,
      page: state.albumDetail.page,
      page_size: pageSize,
    }),
  });

  const items = data.items || [];
  state.albumDetail.total = data.total ?? 0;
  state.photoItems = items.map((p) => p.id);

  const totalPages = Math.max(1, Math.ceil(state.albumDetail.total / pageSize));
  el.albumDetailPageHint.textContent = `第 ${state.albumDetail.page} / ${totalPages} 页 · 共 ${state.albumDetail.total} 张`;
  el.btnAlbumDetailPrev.disabled = state.albumDetail.page <= 1;
  el.btnAlbumDetailNext.disabled = state.albumDetail.page >= totalPages;

  renderAlbumDetailPhotos(items);
}

async function searchAlbumDetailPhotos() {
  syncAlbumDetailFiltersFromForm(true);
  try {
    await fetchAlbumDetailPhotos();
  } catch (err) {
    el.albumDetailPhotos.innerHTML = `<p class="hint">加载失败: ${escapeHtml(err.message)}</p>`;
  }
}

async function loadAlbumDetailPage(page) {
  const pageSize = Number(el.albumDetailPageSize.value) || 48;
  const totalPages = Math.max(1, Math.ceil(state.albumDetail.total / pageSize));
  state.albumDetail.page = Math.max(1, Math.min(totalPages, page));
  try {
    await fetchAlbumDetailPhotos();
  } catch (err) {
    el.albumDetailPhotos.innerHTML = `<p class="hint">加载失败: ${escapeHtml(err.message)}</p>`;
  }
}

async function openAlbumDetailPage(albumId, options = { pushHistory: true }) {
  openDetailView("album", "相册详情", { albumId }, options);
  const switchedAlbum = state.albumDetail.albumId !== albumId;
  state.albumDetail.albumId = albumId;
  if (switchedAlbum) {
    state.albumDetail.page = 1;
    state.albumDetail.filters = {
      keyword: "",
      order: "desc",
      start_time: "",
      end_time: "",
    };
  }
  applyAlbumDetailFiltersToForm();
  el.albumDetailMeta.innerHTML = "<p>状态</p><p>加载中...</p>";
  el.albumDetailPhotos.innerHTML = '<div class="loading">加载相册照片中...</div>';
  try {
    await Promise.all([loadAlbumDetailMeta(albumId), fetchAlbumDetailPhotos()]);
  } catch (err) {
    el.albumDetailMeta.innerHTML = `<p>错误</p><p>${escapeHtml(err.message)}</p>`;
    el.albumDetailPhotos.innerHTML = "";
  }
}

async function saveRemark() {
  if (!state.selectedPhotoId) return;
  el.photoDetailMsg.textContent = "保存中...";
  try {
    await api(`/photos/${state.selectedPhotoId}/remark`, {
      method: "PATCH",
      body: JSON.stringify({ remark: el.photoRemark.value }),
    });
    el.photoDetailMsg.textContent = "备注已保存";
    showToast("备注已保存");
    await fetchAndRenderPhotos();
  } catch (err) {
    el.photoDetailMsg.textContent = `保存失败: ${err.message}`;
    showToast(`保存失败: ${err.message}`, "error");
  }
}

async function addPhotoToAlbum() {
  if (!state.selectedPhotoId) return;
  const albumId = el.photoAlbumSelect.value;
  if (!albumId) {
    el.photoDetailMsg.textContent = "请选择相册";
    return;
  }
  el.photoDetailMsg.textContent = "加入中...";
  try {
    await api("/photos/batch/add-to-album", {
      method: "POST",
      body: JSON.stringify({ album_id: albumId, photo_ids: [state.selectedPhotoId] }),
    });
    el.photoDetailMsg.textContent = "已加入相册";
    showToast("已加入相册");
    await openPhotoDetailPage(state.selectedPhotoId);
  } catch (err) {
    el.photoDetailMsg.textContent = `加入失败: ${err.message}`;
    showToast(`加入失败: ${err.message}`, "error");
  }
}

function startTaskAutoRefresh() {
  if (state.timers.taskAutoRefresh) return;
  state.timers.taskAutoRefresh = window.setInterval(async () => {
    if (state.tab !== "status" || state.timers.taskTickBusy) return;
    state.timers.taskTickBusy = true;
    try {
      await Promise.all([loadTaskOverview(), loadTaskJobs()]);
      if (state.selectedTaskId) {
        await loadTaskDetail(state.selectedTaskId);
      }
      el.taskAutoRefreshHint.textContent = `自动刷新中 · ${new Date().toLocaleTimeString("zh-CN")}`;
    } catch (_err) {
      el.taskAutoRefreshHint.textContent = "自动刷新异常";
    } finally {
      state.timers.taskTickBusy = false;
    }
  }, 3000);
}

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
