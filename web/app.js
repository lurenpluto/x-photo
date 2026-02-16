const state = {
  apiBase: localStorage.getItem("xphoto_api_base") || "http://127.0.0.1:8080/rpc/v1",
  tab: "photos",
  albums: [],
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
  selectedTaskId: null,
  selectedTask: null,
  selectedScanJobId: null,
  taskStatusFilter: "",
  taskJobTypeFilter: "",
  taskTriggerTypeFilter: "",
  detail: {
    active: false,
    type: null,
    tabBeforeOpen: "photos",
    current: null,
    stack: [],
  },
  timers: {
    taskAutoRefresh: null,
    taskTickBusy: false,
  },
};

const $ = (id) => document.getElementById(id);

const el = {
  main: document.querySelector(".main"),
  sideNav: $("sideNav"),
  tabPhotos: $("tab-photos"),
  tabAlbums: $("tab-albums"),
  tabSettings: $("tab-settings"),
  tabStatus: $("tab-status"),

  photoKeyword: $("photoKeyword"),
  btnQuickSearch: $("btnQuickSearch"),
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
  detailTitle: $("detailTitle"),
  photoDetailNav: $("photoDetailNav"),
  photoDetailPage: $("photoDetailPage"),
  albumDetailPage: $("albumDetailPage"),
  albumDetailMeta: $("albumDetailMeta"),
  albumDetailPhotos: $("albumDetailPhotos"),

  btnDrawerPrev: $("btnDrawerPrev"),
  btnDrawerNext: $("btnDrawerNext"),
  photoDetail: $("photoDetail"),
  photoAlbums: $("photoAlbums"),
  photoRemark: $("photoRemark"),
  btnSaveRemark: $("btnSaveRemark"),
  photoAlbumSelect: $("photoAlbumSelect"),
  btnAddToAlbum: $("btnAddToAlbum"),
  photoDetailMsg: $("photoDetailMsg"),
};

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

function setApiBase(value) {
  state.apiBase = value.replace(/\/$/, "");
  localStorage.setItem("xphoto_api_base", state.apiBase);
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
  const tabs = ["photos", "albums", "settings", "status"];
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
}

function closeDetailView() {
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

  state.detail.active = false;
  state.detail.type = null;
  state.detail.current = null;
  state.detail.stack = [];
  el.main.classList.remove("detail-open");
  el.detailView.classList.remove("open");
  el.detailView.setAttribute("aria-hidden", "true");
}

function setTab(tab) {
  const valid = ["photos", "albums", "settings", "status"];
  state.tab = valid.includes(tab) ? tab : "photos";
  if (state.detail.active) {
    state.detail.stack = [];
    closeDetailView();
  }
  location.hash = state.tab;
  updateTabUi();
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
        const timeLabel = (a.created_at || "").slice(0, 10) || "-";
        return `
      <article class="album-card" data-album-id="${a.id}">
        <div class="album-cover" style="background:${visual.background};">
          <p class="item-title">${escapeHtml(title)}</p>
        </div>
        <p class="item-sub">${escapeHtml(timeLabel)} · ${a.auto_created ? "自动" : "手动"} · ${a.id.slice(0, 10)}...</p>
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

async function loadAlbums() {
  el.latestAlbumsRow.innerHTML = '<div class="loading">加载最新相册...</div>';
  if (state.tab === "albums") {
    el.albumGrid.innerHTML = '<div class="loading">加载相册列表...</div>';
  }
  try {
    state.albums = await api("/albums", { method: "GET" });
    renderAlbumCards(el.albumGrid, state.albums);
    renderAlbumCards(el.latestAlbumsRow, state.albums.slice(0, 8));

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
    el.sourceForm.reset();
    await loadSources();
  } catch (err) {
    el.sourceMessage.textContent = `创建失败: ${err.message}`;
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
            <div class="photo-title">${escapeHtml(p.file_name)}</div>
          </div>
          <p class="item-sub">${escapeHtml(p.file_path)}</p>
          <p class="item-sub">sort: ${escapeHtml(p.sort_time)}</p>
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
  state.photoFilters.album_id = el.photoAlbumFilter.value;
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
          <p class="item-sub">${task.id.slice(0, 14)}... · ${task.progress_done}/${task.progress_total ?? "-"}</p>
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
    state.selectedScanJobId = task.scan_job_id || null;
    el.taskDetailHint.textContent = `任务 ${task.id.slice(0, 12)}...`;
    renderTaskFlow(task.status);

    el.taskDetail.innerHTML = [
      ["task_id", task.id],
      ["status", task.status],
      ["trigger_type", task.trigger_type],
      ["scan_job_id", task.scan_job_id || "-"],
      ["scan_status", scanStatus],
      ["progress", `${task.progress_done}/${task.progress_total ?? "-"}`],
      ["error", task.error_message || scanDetail?.error_message || "-"],
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
    await openPhotoDetailPage(state.selectedPhotoId);
  } catch (err) {
    el.photoDetailMsg.textContent = `移除失败: ${err.message}`;
  }
}

async function openPhotoDetailPage(photoId, options = { pushHistory: true }) {
  state.selectedPhotoId = photoId;
  el.photoDetailMsg.textContent = "加载详情...";
  openDetailView("photo", "照片详情", { photoId }, options);
  updateDrawerNavButtons();
  try {
    const data = await api(`/photos/${photoId}`, { method: "GET" });
    const p = data.photo;
    state.selectedPhotoAlbums = data.albums || [];
    const albumText = state.selectedPhotoAlbums.map((a) => a.name).join(" / ") || "-";
    el.photoDetail.innerHTML = [
      ["id", p.id],
      ["file_name", p.file_name],
      ["file_path", p.file_path],
      ["source_id", p.source_id],
      ["sort_time", p.sort_time],
      ["albums", albumText],
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
    el.photoDetailMsg.textContent = "";
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

  const cards = items
    .map((p) => {
      const visual = photoVisualStyle(p);
      return `
      <article class="photo-card" data-album-photo-id="${p.id}">
        <div class="photo-thumb" style="height:${visual.height}px;background:${visual.background};">
          <div class="photo-title">${escapeHtml(p.file_name)}</div>
        </div>
        <p class="item-sub">${escapeHtml(p.file_path)}</p>
        <p class="item-sub">sort: ${escapeHtml(p.sort_time)}</p>
      </article>
    `;
    })
    .join("");

  el.albumDetailPhotos.innerHTML = `<div class="masonry-grid">${cards}</div>`;
  el.albumDetailPhotos.querySelectorAll("[data-album-photo-id]").forEach((node) => {
    node.addEventListener("click", () => {
      const photoId = node.getAttribute("data-album-photo-id");
      if (photoId) {
        openPhotoDetailPage(photoId);
      }
    });
  });
}

async function openAlbumDetailPage(albumId, options = { pushHistory: true }) {
  openDetailView("album", "相册详情", { albumId }, options);
  el.albumDetailMeta.innerHTML = "<p>状态</p><p>加载中...</p>";
  el.albumDetailPhotos.innerHTML = '<div class="loading">加载相册照片中...</div>';
  try {
    const data = await api(`/albums/${albumId}?page=1&page_size=200`, { method: "GET" });
    const album = data.album;
    const photos = data.photos?.items || [];

    el.detailTitle.textContent = `相册详情 · ${album.name}`;
    el.albumDetailMeta.innerHTML = [
      ["id", album.id],
      ["name", album.name],
      ["auto_created", album.auto_created ? "yes" : "no"],
      ["photo_count", String(data.photos?.total ?? photos.length)],
      ["created_at", album.created_at || "-"],
      ["updated_at", album.updated_at || "-"],
    ]
      .map(([k, v]) => `<p>${escapeHtml(k)}</p><p>${escapeHtml(v)}</p>`)
      .join("");

    state.photoItems = photos.map((p) => p.id);
    renderAlbumDetailPhotos(photos);
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
    await fetchAndRenderPhotos();
  } catch (err) {
    el.photoDetailMsg.textContent = `保存失败: ${err.message}`;
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
    await openPhotoDetailPage(state.selectedPhotoId);
  } catch (err) {
    el.photoDetailMsg.textContent = `加入失败: ${err.message}`;
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
    closeDetailView();
    return;
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
  el.apiBase.value = state.apiBase;
  el.apiBase.addEventListener("change", () => setApiBase(el.apiBase.value.trim()));
  el.btnHealth.addEventListener("click", checkHealth);

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

  el.btnQuickSearch.addEventListener("click", searchPhotos);
  el.photoKeyword.addEventListener("keydown", (event) => {
    if (event.key === "Enter") {
      searchPhotos(event);
    }
  });
  el.photoAlbumFilter.addEventListener("change", searchPhotos);
  el.photoOrder.addEventListener("change", searchPhotos);
  el.photoStartDate.addEventListener("change", searchPhotos);
  el.photoEndDate.addEventListener("change", searchPhotos);
  el.photoPageSize.addEventListener("change", searchPhotos);
  el.btnPhotoPrev.addEventListener("click", () => loadPhotoPage(state.photoPage - 1));
  el.btnPhotoNext.addEventListener("click", () => loadPhotoPage(state.photoPage + 1));

  el.btnReloadAlbumsPhotos.addEventListener("click", loadAlbums);
  el.btnReloadAlbums.addEventListener("click", loadAlbums);

  el.sourceForm.addEventListener("submit", createSource);
  el.btnReloadSources.addEventListener("click", loadSources);

  el.btnTaskOverview.addEventListener("click", loadTaskOverview);
  el.btnTaskList.addEventListener("click", loadTaskJobs);
  el.taskFilterForm.addEventListener("submit", loadTaskJobs);
  el.btnCancelTask.addEventListener("click", cancelTask);
  el.btnRetryTask.addEventListener("click", retryTask);
  el.btnCancelScan.addEventListener("click", cancelScan);
  el.btnRetryScan.addEventListener("click", retryScan);

  el.btnDetailBack.addEventListener("click", closeDetailView);
  el.btnDrawerPrev.addEventListener("click", () => moveDrawerPhoto(-1));
  el.btnDrawerNext.addEventListener("click", () => moveDrawerPhoto(1));
  el.btnSaveRemark.addEventListener("click", saveRemark);
  el.btnAddToAlbum.addEventListener("click", addPhotoToAlbum);

  document.addEventListener("keydown", onKeydown);
  bindHorizontalDragScroll(el.latestAlbumsRow);
}

async function boot() {
  readPhotoStateFromUrl();
  bindEvents();
  startTaskAutoRefresh();
  initTabFromHash();
  setTab(state.tab || "photos");
  await checkHealth();
  await Promise.all([loadAlbums(), loadSources(), loadTaskOverview(), loadTaskJobs()]);
  syncPhotoFiltersFromForm(false);
  await fetchAndRenderPhotos();
}

boot();
