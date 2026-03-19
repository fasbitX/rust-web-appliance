// ═══════════════════════════════════════════════════════════════════
// Rust Web Appliance — Frontend Controller
// Drop your own JS here or replace this file entirely.
// ═══════════════════════════════════════════════════════════════════

const API = '';  // Same origin — no prefix needed

// ── Boot: health check + info ──────────────────────────────────────
document.addEventListener('DOMContentLoaded', async () => {
    await checkHealth();
    await loadInfo();
    await refreshKeys();
});

async function checkHealth() {
    const el = document.getElementById('status');
    try {
        const res = await fetch(`${API}/api/health`);
        const data = await res.json();
        if (data.status === 'ok') {
            el.textContent = 'OPERATIONAL';
            el.className = 'status ok';
        } else {
            el.textContent = 'DEGRADED';
            el.className = 'status error';
        }
    } catch {
        el.textContent = 'UNREACHABLE';
        el.className = 'status error';
    }
}

async function loadInfo() {
    const el = document.getElementById('info');
    try {
        const res = await fetch(`${API}/api/info`);
        const data = await res.json();
        el.innerHTML = [
            `<span class="label">name:</span>    <span class="value">${data.name}</span>`,
            `<span class="label">version:</span> <span class="value">${data.version}</span>`,
            `<span class="label">os:</span>      <span class="value">${data.os}</span>`,
        ].join('\n');
        el.classList.remove('hidden');
    } catch {
        // Silent — info card stays hidden
    }
}

// ── Key-Value Operations ────────────────────────────────────────────

async function kvPut() {
    const key = document.getElementById('kv-key').value.trim();
    const value = document.getElementById('kv-value').value.trim();
    const out = document.getElementById('kv-result');

    if (!key) { out.textContent = 'Error: key is required'; return; }
    if (!value) { out.textContent = 'Error: value is required'; return; }

    try {
        const res = await fetch(`${API}/api/kv/${encodeURIComponent(key)}`, {
            method: 'PUT',
            body: value,
        });
        const data = await res.json();
        out.textContent = `PUT ${key} → ${JSON.stringify(data, null, 2)}`;
        await refreshKeys();
    } catch (e) {
        out.textContent = `Error: ${e.message}`;
    }
}

async function kvGet() {
    const key = document.getElementById('kv-key').value.trim();
    const out = document.getElementById('kv-result');

    if (!key) { out.textContent = 'Error: key is required'; return; }

    try {
        const res = await fetch(`${API}/api/kv/${encodeURIComponent(key)}`);
        const data = await res.json();
        out.textContent = `GET ${key} → ${JSON.stringify(data, null, 2)}`;
    } catch (e) {
        out.textContent = `Error: ${e.message}`;
    }
}

async function kvDelete() {
    const key = document.getElementById('kv-key').value.trim();
    const out = document.getElementById('kv-result');

    if (!key) { out.textContent = 'Error: key is required'; return; }

    try {
        const res = await fetch(`${API}/api/kv/${encodeURIComponent(key)}`, {
            method: 'DELETE',
        });
        const data = await res.json();
        out.textContent = `DELETE ${key} → ${JSON.stringify(data, null, 2)}`;
        await refreshKeys();
    } catch (e) {
        out.textContent = `Error: ${e.message}`;
    }
}

async function refreshKeys() {
    const el = document.getElementById('kv-keys');
    try {
        const res = await fetch(`${API}/api/kv`);
        const data = await res.json();
        if (data.keys && data.keys.length > 0) {
            el.textContent = data.keys.join('\n');
        } else {
            el.textContent = '(empty)';
        }
    } catch {
        el.textContent = '(could not load keys)';
    }
}
