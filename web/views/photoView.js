export function createPhotoView({
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
}) {
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

  function togglePhotoHelp(force) {
    if (!el.photoHelpPanel) return;
    const willShow = typeof force === "boolean" ? force : el.photoHelpPanel.classList.contains("hidden");
    el.photoHelpPanel.classList.toggle("hidden", !willShow);
    el.photoHelpPanel.setAttribute("aria-hidden", willShow ? "false" : "true");
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
            <img class="photo-thumb-img" data-photo-thumb-id="${p.id}" src="${state.apiBase}/photos/${p.id}/thumb?max_edge=560" alt="${escapeHtml(p.file_name || "照片")}" loading="lazy" />
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
            <img class="photo-thumb-img" data-favorite-thumb-id="${p.id}" src="${state.apiBase}/photos/${p.id}/thumb?max_edge=560" alt="${escapeHtml(p.file_name || "照片")}" loading="lazy" />
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

  return {
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
  };
}
