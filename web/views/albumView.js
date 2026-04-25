export function createAlbumView({
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
  getOpenPhotoDetailPage,
}) {
  function renderAlbumCards(target, albums) {
    if (!albums.length) {
      target.innerHTML = '<p class="hint">暂无相册。</p>';
      return;
    }
    target.innerHTML = albums
      .map((a) => {
        const visual = albumVisualStyle(a);
        const title = a.name || "未命名相册";
        const count = Number(a.photo_count ?? state.albumPhotoCounts[a.id]);
        const countLabel = Number.isFinite(count) ? `${count} 项` : "...";
        return `
      <article class="album-card" data-album-id="${a.id}">
        <div class="album-cover" style="background:${visual.background};">
          ${
            a.cover_photo_id
              ? `<img class="album-cover-img" data-album-cover-id="${a.id}" src="${state.apiBase}/photos/${a.cover_photo_id}/thumb?max_edge=720" alt="${escapeHtml(title)}" loading="lazy" />
                 <span class="album-cover-missing hidden" data-album-cover-missing-id="${a.id}">封面缺失</span>`
              : ""
          }
        </div>
        <p class="album-name">${escapeHtml(title)}</p>
        <p class="album-count">${escapeHtml(countLabel)}</p>
      </article>
    `;
      })
      .join("");

    target.querySelectorAll(".album-card[data-album-id]").forEach((item) => {
      item.addEventListener("click", () => {
        const albumId = item.getAttribute("data-album-id");
        if (albumId) {
          openAlbumDetailPage(albumId);
        }
      });
    });

    target.querySelectorAll("[data-album-cover-id]").forEach((img) => {
      img.addEventListener("error", () => {
        img.classList.add("hidden");
        const albumId = img.getAttribute("data-album-cover-id");
        if (!albumId) return;
        const missing = target.querySelector(`[data-album-cover-missing-id="${albumId}"]`);
        if (missing) {
          missing.classList.remove("hidden");
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
      state.albumPhotoCounts = Object.fromEntries(
        state.albums
          .map((a) => [a.id, Number(a.photo_count)])
          .filter(([, count]) => Number.isFinite(count)),
      );
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
          <button class="album-cover-set-btn" type="button" data-set-cover-photo-id="${p.id}" title="设为相册封面">设为封面</button>
          <div class="photo-thumb" style="height:${visual.height}px;background:${visual.background};">
            <img class="photo-thumb-img" data-album-thumb-id="${p.id}" src="${state.apiBase}/photos/${p.id}/thumb?max_edge=560" alt="${escapeHtml(p.file_name || "照片")}" loading="lazy" />
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
          getOpenPhotoDetailPage()(photoId);
        }
      });
    });

    el.albumDetailPhotos.querySelectorAll("[data-set-cover-photo-id]").forEach((btn) => {
      btn.addEventListener("click", async (event) => {
        event.stopPropagation();
        const photoId = btn.getAttribute("data-set-cover-photo-id");
        const albumId = state.albumDetail.albumId;
        if (!photoId || !albumId) return;
        btn.disabled = true;
        try {
          await api(`/albums/${albumId}/cover`, {
            method: "PATCH",
            body: JSON.stringify({ cover_photo_id: photoId }),
          });
          showToast("已设为相册封面");
          await loadAlbums();
          await loadAlbumDetailMeta(albumId);
        } catch (err) {
          showToast(`设置封面失败: ${err.message}`, "error");
        } finally {
          btn.disabled = false;
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
      ["封面照片", album.cover_photo_id || "-"],
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

  return {
    loadAlbumDetailPage,
    loadAlbums,
    openAlbumDetailPage,
    searchAlbumDetailPhotos,
  };
}
