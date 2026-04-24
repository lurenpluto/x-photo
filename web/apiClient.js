export function createApiClient({ getBaseUrl, timeoutMs = 15000 }) {
  async function request(path, options = {}) {
    const controller = new AbortController();
    const timeout = window.setTimeout(() => controller.abort(), timeoutMs);
    try {
      const res = await fetch(`${getBaseUrl()}${path}`, {
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

  return { request };
}
