const esc = (s) => { const d = document.createElement('div'); d.textContent = s; return d.innerHTML; };

let onAuthLost = null;

async function api(path, opts = {}) {
  const { json, method, body, headers: extra } = opts;
  const h = { 'X-NF-Request': '1', ...extra };
  let reqBody = body;
  if (json) {
    h['Content-Type'] = 'application/json';
    reqBody = JSON.stringify(json);
  }
  const res = await fetch(path, { method: method || (json ? 'POST' : 'GET'), headers: h, body: reqBody, credentials: 'same-origin' });
  if (res.status === 401 && path !== '/api/login') {
    if (onAuthLost) onAuthLost();
    throw new ApiError('Session expired', { status: 401 });
  }
  if (!res.ok) {
    let detail;
    try { detail = await res.json(); } catch { detail = {}; }
    throw new ApiError(detail.error || detail.message || res.statusText, { status: res.status, ...detail });
  }
  if (res.status === 204) return null;
  return res.json();
}

class ApiError extends Error {
  constructor(msg, detail = {}) {
    super(msg);
    this.status = detail.status;
    this.code = detail.code;
    this.detail = detail;
  }
}

function uploadFile(url, file, onProgress) {
  return new Promise((resolve, reject) => {
    const xhr = new XMLHttpRequest();
    xhr.open('POST', url);
    xhr.setRequestHeader('X-NF-Request', '1');
    xhr.setRequestHeader('X-Filename', encodeURIComponent(file.name).slice(0, 180));
    xhr.withCredentials = true;
    if (onProgress) xhr.upload.onprogress = (e) => { if (e.lengthComputable) onProgress(e.loaded / e.total); };
    xhr.onload = () => {
      if (xhr.status >= 200 && xhr.status < 300) {
        try { resolve(JSON.parse(xhr.responseText)); } catch { resolve(null); }
      } else {
        let detail = {};
        try { detail = JSON.parse(xhr.responseText); } catch { /* */ }
        reject(new ApiError(detail.error || xhr.statusText, { status: xhr.status, ...detail }));
      }
    };
    xhr.onerror = () => reject(new ApiError('Network error'));
    xhr.send(file);
  });
}

export { api, ApiError, uploadFile, esc, onAuthLost };
export function setAuthHandler(fn) { onAuthLost = fn; }
