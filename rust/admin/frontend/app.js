/* global __TAURI__ */
'use strict';

const { invoke } = window.__TAURI__.core;

// ── Tab switching ──────────────────────────────────────────────────────────

document.querySelectorAll('.tab').forEach(tab => {
  tab.addEventListener('click', () => {
    document.querySelectorAll('.tab').forEach(t => t.classList.remove('active'));
    document.querySelectorAll('.panel').forEach(p => p.classList.remove('active'));
    tab.classList.add('active');
    document.getElementById(tab.dataset.panel).classList.add('active');
  });
});

// ── Log ────────────────────────────────────────────────────────────────────

const logEl = document.getElementById('log');

function log(msg) {
  const ts = new Date().toLocaleTimeString();
  logEl.textContent += `[${ts}] ${msg}\n`;
  logEl.scrollTop = logEl.scrollHeight;
}

function logClear() {
  logEl.textContent = '';
}

// ── Config form helpers ────────────────────────────────────────────────────

function configFromForm() {
  return {
    client_dir: document.getElementById('cfg-client-dir').value,
    patch_dir:  document.getElementById('cfg-patch-dir').value,
    plist_path: document.getElementById('cfg-plist-path').value,
    ftp_host:   document.getElementById('cfg-ftp-host').value,
    ftp_user:   document.getElementById('cfg-ftp-user').value,
    use_sftp:   document.getElementById('cfg-use-sftp').checked,
  };
}

function configToForm(cfg) {
  document.getElementById('cfg-client-dir').value  = cfg.client_dir ?? '';
  document.getElementById('cfg-patch-dir').value   = cfg.patch_dir  ?? 'Patch';
  document.getElementById('cfg-plist-path').value  = cfg.plist_path ?? 'PList.Bin';
  document.getElementById('cfg-ftp-host').value    = cfg.ftp_host   ?? '';
  document.getElementById('cfg-ftp-user').value    = cfg.ftp_user   ?? '';
  document.getElementById('cfg-use-sftp').checked  = cfg.use_sftp   ?? false;
}

// ── Config buttons ─────────────────────────────────────────────────────────

document.getElementById('btn-save-config').addEventListener('click', async () => {
  try {
    await invoke('save_patch_config', { config: configFromForm() });
    log('Config saved.');
  } catch (err) {
    log(`Error saving config: ${err}`);
  }
});

document.getElementById('btn-load-config').addEventListener('click', async () => {
  try {
    const cfg = await invoke('read_patch_config');
    configToForm(cfg);
    log('Config loaded.');
  } catch (err) {
    log(`Error loading config: ${err}`);
  }
});

// ── Scan ───────────────────────────────────────────────────────────────────

document.getElementById('btn-scan').addEventListener('click', async () => {
  const path = document.getElementById('cfg-client-dir').value.trim();
  if (!path) { log('Error: Client Directory is required.'); return; }
  logClear();
  log(`Scanning ${path} ...`);
  try {
    const files = await invoke('scan_client_dir', { path });
    log(`Found ${files.length} file(s):`);
    const preview = files.slice(0, 20);
    preview.forEach(f => log(`  ${f.path}  [${f.checksum.slice(0, 8)}…]`));
    if (files.length > 20) log(`  … and ${files.length - 20} more`);
  } catch (err) {
    log(`Error: ${err}`);
  }
});

// ── Build patches ──────────────────────────────────────────────────────────

document.getElementById('btn-build').addEventListener('click', async () => {
  const cfg = configFromForm();
  if (!cfg.client_dir) { log('Error: Client Directory is required.'); return; }
  logClear();
  log('Building patches...');
  try {
    const result = await invoke('build_patches', {
      client_dir: cfg.client_dir,
      patch_dir:  cfg.patch_dir,
      plist_path: cfg.plist_path,
    });
    log(`Done: ${result.total_files} total file(s), ${result.compressed} compressed.`);
    log(`PList.Bin written to: ${result.plist_path}`);
  } catch (err) {
    log(`Error: ${err}`);
  }
});

// ── Restore config on load ─────────────────────────────────────────────────

window.addEventListener('load', async () => {
  try {
    const cfg = await invoke('read_patch_config');
    configToForm(cfg);
  } catch (_) {
    // first run — no saved config yet
  }
});
