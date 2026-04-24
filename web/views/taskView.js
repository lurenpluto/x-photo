export function createTaskView({ state, el, api, escapeHtml }) {
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
      // Status panels can still render without source display names.
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

  function parseIsoMs(value) {
    if (!value) return null;
    const ms = Date.parse(value);
    return Number.isFinite(ms) ? ms : null;
  }

  function formatCompactDuration(seconds) {
    if (!Number.isFinite(seconds) || seconds < 0) return "-";
    const s = Math.round(seconds);
    const d = Math.floor(s / 86400);
    const h = Math.floor((s % 86400) / 3600);
    const m = Math.floor((s % 3600) / 60);
    const sec = s % 60;
    if (d > 0) return `${d}d${h}h`;
    if (h > 0) return `${h}h${m}m`;
    if (m > 0) return `${m}m${sec}s`;
    return `${sec}s`;
  }

  function calcTaskThroughput(task) {
    const done = Number(task.progress_done || 0);
    const startedMs = parseIsoMs(task.started_at);
    if (!Number.isFinite(done) || done <= 0 || !startedMs) return null;
    const elapsedSec = Math.max(1, (Date.now() - startedMs) / 1000);
    return done / elapsedSec;
  }

  function taskThroughputText(task) {
    const speed = calcTaskThroughput(task);
    if (!speed) return "速度: -";
    return `速度: ${speed.toFixed(1)} 张/秒`;
  }

  function taskEtaText(task) {
    const done = Number(task.progress_done || 0);
    const total = Number(task.progress_total || 0);
    if (!Number.isFinite(total) || total <= 0 || done >= total) return "预计剩余: -";
    const speed = calcTaskThroughput(task);
    if (!speed || speed <= 0) return "预计剩余: -";
    const remainingSec = (total - done) / speed;
    return `预计剩余: ${formatCompactDuration(remainingSec)}`;
  }

  function taskElapsedText(task) {
    const startedMs = parseIsoMs(task.started_at);
    if (!startedMs) return "已运行: -";
    return `已运行: ${formatCompactDuration((Date.now() - startedMs) / 1000)}`;
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
          <p class="item-sub">${escapeHtml(taskThroughputText(task))} · ${escapeHtml(taskEtaText(task).replace("预计剩余: ", "剩余: "))}</p>
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
        ["throughput", taskThroughputText(task)],
        ["eta", taskEtaText(task)],
        ["elapsed", taskElapsedText(task)],
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

  return {
    cancelScan,
    cancelTask,
    ensureSourceCache,
    loadTaskDetail,
    loadTaskJobs,
    loadTaskOverview,
    retryScan,
    retryTask,
    startTaskAutoRefresh,
  };
}
