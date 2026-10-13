'use strict';

const { invoke, Channel } = window.__TAURI__.core;
const { open: openDialog, message, ask } = window.__TAURI__.dialog;
const { getCurrentWebview } = window.__TAURI__.webview;
const { getCurrentWindow } = window.__TAURI__.window;

const $ = (id) => document.getElementById(id);

const IMAGE_EXTS = ['jpg', 'jpeg', 'png', 'webp', 'bmp', 'tif', 'tiff', 'gif'];
const SETTINGS_KEY = 'imaginify.settings.v1';
const RESULTS_SHOWN = 200;
const MIN_CROP_PX = 4;

const PRESETS = [
  ['1920w (HD)', 1920, 0],
  ['1280w (web)', 1280, 0],
  ['800w (thumb)', 800, 0],
  ['Instagram 1080', 1080, 1080],
  ['Facebook 2048w', 2048, 0],
  ['1080p HD', 1920, 1080],
];

const state = {
  files: [], // { path, name }
  known: new Set(),
  names: new Map(), // path -> file name, for messages
  checked: new Set(),
  selected: -1,
  crops: new Map(), // path -> { x, y, w, h } as fractions of the upright image
  dims: null, // full-size { w, h } of the selected image
  previewUrl: null,
  previewToken: 0,
  running: false,
};

/** The crop drag in progress: { path, token, start, rect }. */
let drag = null;
let previewTimer = 0;
const PREVIEW_DEBOUNCE_MS = 120;

// ---------- helpers ----------

function human(n) {
  const units = ['B', 'KB', 'MB', 'GB'];
  let v = Math.abs(n);
  let i = 0;
  while (v >= 1024 && i < units.length - 1) { v /= 1024; i++; }
  return `${n < 0 ? '-' : ''}${v.toFixed(1)} ${units[i]}`;
}

/** Display name for a path: the name the scan reported, else the last path segment. */
function nameOf(p) {
  return state.names.get(p) ?? String(p).split(navigator.userAgent.includes('Windows') ? /[\\/]/ : '/').pop();
}

function setStatus(text) {
  $('status').textContent = text;
}

const clamp01 = (v) => Math.min(1, Math.max(0, v));

// ---------- settings (remembered between launches) ----------

function readSettingsForm() {
  return {
    width: $('width').value.trim(),
    height: $('height').value.trim(),
    keepAspect: $('keep-aspect').checked,
    noEnlarge: $('no-enlarge').checked,
    quality: Number($('quality').value),
    format: $('format').value,
    suffix: $('suffix').value,
    keepIfLarger: $('keep-if-larger').checked,
    outDir: $('out-dir').value,
    cropRatio: $('crop-ratio').value,
  };
}

function saveSettings() {
  try { localStorage.setItem(SETTINGS_KEY, JSON.stringify(readSettingsForm())); } catch { /* storage unavailable */ }
}

async function loadSettings() {
  let saved = {};
  try { saved = JSON.parse(localStorage.getItem(SETTINGS_KEY) || '{}') || {}; } catch { saved = {}; }
  const set = (id, prop, value) => { if (value !== undefined && value !== null) $(id)[prop] = value; };
  set('width', 'value', saved.width);
  set('height', 'value', saved.height);
  set('keep-aspect', 'checked', saved.keepAspect);
  set('no-enlarge', 'checked', saved.noEnlarge);
  set('quality', 'value', saved.quality);
  set('format', 'value', saved.format);
  set('suffix', 'value', saved.suffix);
  set('keep-if-larger', 'checked', saved.keepIfLarger);
  set('crop-ratio', 'value', saved.cropRatio);
  $('quality-out').textContent = $('quality').value;
  $('out-dir').value = saved.outDir || await invoke('default_output_dir');
}

/** Settings for the Rust side, or null after telling the user what is wrong. */
async function settingsForProcessing() {
  const f = readSettingsForm();
  const size = (v) => (v === '' ? 0 : /^\d+$/.test(v) ? Number(v) : NaN);
  const width = size(f.width);
  const height = size(f.height);
  if (!Number.isSafeInteger(width) || !Number.isSafeInteger(height) || width > 100000 || height > 100000) {
    await message('Width and height must be whole numbers of pixels (or blank).', { title: 'Invalid size', kind: 'error' });
    return null;
  }
  return {
    width, height,
    keepAspect: f.keepAspect,
    noEnlarge: f.noEnlarge,
    quality: f.quality,
    format: f.format,
    suffix: f.suffix,
    keepIfLarger: f.keepIfLarger,
  };
}

// ---------- file list ----------

function updateCounts() {
  const n = state.checked.size;
  $('count').textContent = `${n} of ${state.files.length} selected`;
  $('process').textContent = state.running ? 'Processing…' : `Process selected (${n})`;
}

function renderList() {
  const frag = document.createDocumentFragment();
  state.files.forEach((f, i) => {
    const li = document.createElement('li');
    li.dataset.index = String(i);
    li.title = f.path;
    if (i === state.selected) li.classList.add('selected');
    const cb = document.createElement('input');
    cb.type = 'checkbox';
    cb.checked = state.checked.has(f.path);
    const name = document.createElement('span');
    name.className = 'name';
    name.textContent = f.name;
    const mark = document.createElement('span');
    mark.className = 'cropped';
    mark.textContent = 'cropped';
    mark.hidden = !state.crops.has(f.path);
    li.append(cb, name, mark);
    frag.append(li);
  });
  $('file-list').replaceChildren(frag);
  $('empty').hidden = state.files.length > 0;
  updateCounts();
}

function rowAt(i) {
  return $('file-list').children[i] || null;
}

function refreshCropMark(path) {
  const i = state.files.findIndex((f) => f.path === path);
  const mark = rowAt(i)?.querySelector('.cropped');
  if (mark) mark.hidden = !state.crops.has(path);
}

async function addPaths(paths) {
  if (!paths || !paths.length) return;
  setStatus('Looking for images…');
  let entries;
  try {
    entries = await invoke('scan_paths', { paths });
  } catch (e) {
    setStatus(`Could not read the dropped items: ${e}`);
    return;
  }
  let added = 0;
  for (const entry of entries) {
    if (state.known.has(entry.path)) continue;
    state.known.add(entry.path);
    state.names.set(entry.path, entry.name);
    state.checked.add(entry.path);
    state.files.push(entry);
    added++;
  }
  renderList();
  setStatus(added
    ? `Added ${added} image${added === 1 ? '' : 's'}. Total: ${state.files.length}`
    : 'Nothing added: no new supported images found.');
  if (added && state.selected < 0) select(0);
}

function select(i) {
  if (i < 0 || i >= state.files.length) return;
  rowAt(state.selected)?.classList.remove('selected');
  state.selected = i;
  const row = rowAt(i);
  row?.classList.add('selected');
  row?.scrollIntoView({ block: 'nearest' });
  // Wait for the selection to settle (arrow-key scrolling) before decoding.
  clearTimeout(previewTimer);
  showNoPreview('Loading…');
  const file = state.files[i];
  previewTimer = setTimeout(() => loadPreview(file), PREVIEW_DEBOUNCE_MS);
}

function removeSelected() {
  const i = state.selected;
  if (i < 0) return;
  const [gone] = state.files.splice(i, 1);
  state.known.delete(gone.path);
  state.names.delete(gone.path);
  state.checked.delete(gone.path);
  state.crops.delete(gone.path);
  state.selected = -1;
  renderList();
  if (state.files.length) select(Math.min(i, state.files.length - 1));
  else showNoPreview('');
  setStatus(`Total: ${state.files.length}`);
}

function clearAll() {
  state.files = [];
  state.known.clear();
  state.names.clear();
  state.checked.clear();
  state.crops.clear();
  state.selected = -1;
  renderList();
  showNoPreview('');
  setStatus('Cleared');
}

// ---------- preview + crop ----------

function showNoPreview(text) {
  clearTimeout(previewTimer);
  state.previewToken++;
  state.dims = null;
  drag = null;
  $('frame').hidden = true;
  $('stage-msg').textContent = text;
  if (state.previewUrl) { URL.revokeObjectURL(state.previewUrl); state.previewUrl = null; }
  updateCropUi();
}

async function loadPreview(file) {
  // Retire the previous image straight away: until the new preview arrives
  // there is nothing to crop, so a drag cannot land on the wrong file.
  showNoPreview('Loading…');
  const token = state.previewToken;
  try {
    const buf = await invoke('preview', { path: file.path });
    if (token !== state.previewToken) return;
    const view = new DataView(buf);
    const w = view.getUint32(0, true);
    const h = view.getUint32(4, true);
    const type = view.getUint8(8) === 1 ? 'image/png' : 'image/jpeg';
    if (state.previewUrl) URL.revokeObjectURL(state.previewUrl);
    state.previewUrl = URL.createObjectURL(new Blob([new Uint8Array(buf, 9)], { type }));
    state.dims = { w, h };
    $('preview-img').src = state.previewUrl;
    $('frame').hidden = false;
    $('stage-msg').textContent = '';
    layoutFrame();
    updateCropUi();
    setStatus(`${file.name}   |   ${w} × ${h}px`);
  } catch (e) {
    if (token !== state.previewToken) return;
    showNoPreview(`Cannot open this image: ${e}`);
  }
}

/** Fit the preview into the stage without enlarging past the real size. */
function layoutFrame() {
  if (!state.dims) return;
  const stage = $('stage');
  const pad = 16;
  const scale = Math.min((stage.clientWidth - pad) / state.dims.w, (stage.clientHeight - pad) / state.dims.h, 1);
  $('frame').style.width = `${Math.max(1, Math.floor(state.dims.w * scale))}px`;
  $('frame').style.height = `${Math.max(1, Math.floor(state.dims.h * scale))}px`;
}

function currentPath() {
  return state.selected >= 0 ? state.files[state.selected].path : null;
}

/**
 * The crop as it will be applied to the shown image: a locked shape is held
 * in pixels, shrinking the longer side around the centre (same rule as
 * `crop_rect` in core/src/geometry.rs), so a 1:1 crop copied to a portrait
 * photo stays square.
 */
function fitted(c) {
  if (!c || !c.ratio || !state.dims) return c;
  const { w: W, h: H } = state.dims;
  const pw = c.w * W;
  const ph = c.h * H;
  if (pw / ph > c.ratio) {
    const nw = ph * c.ratio;
    return { ...c, x: c.x + (pw - nw) / 2 / W, w: nw / W };
  }
  if (pw / ph < c.ratio) {
    const nh = pw / c.ratio;
    return { ...c, y: c.y + (ph - nh) / 2 / H, h: nh / H };
  }
  return c;
}

function drawCrop(c) {
  const box = $('crop-box');
  c = fitted(c);
  if (!c) { box.hidden = true; return; }
  box.hidden = false;
  box.style.left = `${c.x * 100}%`;
  box.style.top = `${c.y * 100}%`;
  box.style.width = `${c.w * 100}%`;
  box.style.height = `${c.h * 100}%`;
}

function updateCropUi() {
  const path = currentPath();
  const c = path && state.dims ? state.crops.get(path) : null;
  drawCrop(c);
  $('crop-clear').disabled = !c;
  $('crop-all').disabled = !c;
  if (c && state.dims) {
    const { w, h } = state.dims;
    const f = fitted(c);
    const opt = c.ratio ? [...$('crop-ratio').options].find((o) => Math.abs(Number(o.value) - c.ratio) < 1e-3) : null;
    const shape = c.ratio ? `  (shape ${opt ? opt.textContent : 'locked'})` : '';
    $('crop-info').textContent =
      `Crop: ${Math.round(f.w * w)} × ${Math.round(f.h * h)}px from ${Math.round(f.x * w)}, ${Math.round(f.y * h)}${shape}`;
  } else {
    $('crop-info').textContent = 'No crop';
  }
}

function pointerFraction(e) {
  const r = $('frame').getBoundingClientRect();
  return { x: clamp01((e.clientX - r.left) / r.width), y: clamp01((e.clientY - r.top) / r.height) };
}

/** Rectangle from the drag start to the pointer, held to the chosen shape and inside the image. */
function cropFromDrag(a, b) {
  let dx = b.x - a.x;
  let dy = b.y - a.y;
  const ratio = Number($('crop-ratio').value);
  if (ratio > 0 && state.dims) {
    const k = state.dims.w / state.dims.h / ratio; // fraction-height per fraction-width
    const sx = Math.sign(dx) || 1;
    const sy = Math.sign(dy) || 1;
    let fw = Math.abs(dx);
    let fh = Math.abs(dy);
    if (fh < fw * k) fh = fw * k; else fw = fh / k;
    const maxW = sx > 0 ? 1 - a.x : a.x;
    const maxH = sy > 0 ? 1 - a.y : a.y;
    if (fw > maxW) { fw = maxW; fh = fw * k; }
    if (fh > maxH) { fh = maxH; fw = fh / k; }
    dx = sx * fw;
    dy = sy * fh;
  }
  return { x: Math.min(a.x, a.x + dx), y: Math.min(a.y, a.y + dy), w: Math.abs(dx), h: Math.abs(dy) };
}


function onPointerDown(e) {
  if (!state.dims || e.button !== 0) return;
  $('frame').setPointerCapture(e.pointerId);
  drag = { path: currentPath(), token: state.previewToken, start: pointerFraction(e), rect: null };
  drawCrop(null);
}

function onPointerMove(e) {
  if (!drag) return;
  drag.rect = cropFromDrag(drag.start, pointerFraction(e));
  drawCrop(drag.rect);
}

function onPointerUp() {
  if (!drag) return;
  const { path, token, rect: c } = drag;
  drag = null;
  if (!path || path !== currentPath() || token !== state.previewToken) return;
  const big = c && state.dims && c.w * state.dims.w >= MIN_CROP_PX && c.h * state.dims.h >= MIN_CROP_PX;
  const ratio = Number($('crop-ratio').value);
  if (big) state.crops.set(path, { ...c, ratio: ratio > 0 ? ratio : null });
  else state.crops.delete(path);
  refreshCropMark(path);
  updateCropUi();
}

function cropForAllTicked() {
  const c = state.crops.get(currentPath());
  if (!c) return;
  for (const p of state.checked) state.crops.set(p, { ...c });
  renderList();
  setStatus(`Crop applied to ${state.checked.size} ticked image${state.checked.size === 1 ? '' : 's'} (same position; a locked shape stays that shape on every image).`);
}

function clearCrop() {
  const path = currentPath();
  if (!path) return;
  state.crops.delete(path);
  refreshCropMark(path);
  updateCropUi();
}

// ---------- processing ----------

function setRunning(on) {
  state.running = on;
  $('process').disabled = on;
  $('cancel').hidden = !on;
  $('cancel').disabled = false;
  updateCounts();
}

async function processSelected() {
  if (state.running) return;
  const targets = state.files.filter((f) => state.checked.has(f.path));
  if (!targets.length) {
    await message('Tick the files you want to process, or click “Select all”.', { title: 'Nothing selected' });
    return;
  }
  const settings = await settingsForProcessing();
  if (!settings) return;
  const outDir = $('out-dir').value.trim();
  if (!outDir) {
    await message('Choose an output folder first.', { title: 'Output folder', kind: 'warning' });
    return;
  }
  const jobs = targets.map((f) => ({ path: f.path, crop: state.crops.get(f.path) ?? null }));

  setRunning(true);
  $('progress').value = 0;
  setStatus(`Processing ${jobs.length} image${jobs.length === 1 ? '' : 's'}…`);
  const onProgress = new Channel();
  onProgress.onmessage = (p) => {
    $('progress').value = p.done / p.total;
    const o = p.outcome;
    let text = `[${p.done}/${p.total}] ${nameOf(o.src)}`;
    if (o.error) text += `: failed (${o.error})`;
    else if (o.out) text += `  →  ${nameOf(o.out)}   (${human(o.sizeIn)} → ${human(o.sizeOut)})`;
    else if (o.cancelled) text += ': cancelled';
    setStatus(text);
  };
  try {
    const outcomes = await invoke('process', { jobs, settings, outDir, onProgress });
    showResults(outcomes, outDir);
  } catch (e) {
    setStatus(String(e));
    await message(String(e), { title: 'Could not process', kind: 'error' });
  } finally {
    setRunning(false);
  }
}

function showResults(outcomes, outDir) {
  const ok = outcomes.filter((o) => o.out);
  const failed = outcomes.filter((o) => o.error);
  const cancelled = outcomes.filter((o) => o.cancelled);
  const noted = ok.filter((o) => o.note);
  const sizeIn = ok.reduce((s, o) => s + o.sizeIn, 0);
  const sizeOut = ok.reduce((s, o) => s + o.sizeOut, 0);
  const saved = sizeIn - sizeOut;
  const pct = sizeIn ? (saved / sizeIn) * 100 : 0;

  const parts = [`${ok.length} saved`];
  if (failed.length) parts.push(`${failed.length} failed`);
  if (cancelled.length) parts.push(`${cancelled.length} cancelled`);
  const summary = `${parts.join(', ')}.  ${saved >= 0 ? 'Saved' : 'Grew by'} ${human(Math.abs(saved))} (${Math.abs(pct).toFixed(1)}%).`;
  setStatus(`Done. ${summary}`);

  $('results-title').textContent = failed.length ? 'Finished with errors' : cancelled.length ? 'Cancelled' : 'Done';
  $('results-summary').textContent = summary;
  const frag = document.createDocumentFragment();
  const rows = [
    ...failed.map((o) => [`${nameOf(o.src)}: ${o.error}`, 'err']),
    ...noted.map((o) => [`${nameOf(o.src)}: ${o.note}`, '']),
  ];
  for (const [text, cls] of rows.slice(0, RESULTS_SHOWN)) {
    const li = document.createElement('li');
    li.textContent = text;
    if (cls) li.className = cls;
    frag.append(li);
  }
  if (rows.length > RESULTS_SHOWN) {
    const li = document.createElement('li');
    li.textContent = `…and ${rows.length - RESULTS_SHOWN} more`;
    frag.append(li);
  }
  $('results-list').replaceChildren(frag);
  $('results-list').hidden = rows.length === 0;
  $('results-open').onclick = () => invoke('open_folder', { path: outDir }).catch((e) => setStatus(String(e)));
  $('results-open').hidden = ok.length === 0;
  $('results').showModal();
}

// ---------- wiring ----------

function wire() {
  $('add-files').addEventListener('click', async () => {
    const picked = await openDialog({ multiple: true, title: 'Choose images', filters: [{ name: 'Images', extensions: IMAGE_EXTS }] });
    if (picked) addPaths(Array.isArray(picked) ? picked : [picked]);
  });
  $('add-folder').addEventListener('click', async () => {
    const picked = await openDialog({ directory: true, multiple: true, title: 'Choose folders of images' });
    if (picked) addPaths(Array.isArray(picked) ? picked : [picked]);
  });
  $('remove').addEventListener('click', removeSelected);
  $('clear').addEventListener('click', clearAll);
  $('select-all').addEventListener('click', () => { state.files.forEach((f) => state.checked.add(f.path)); renderList(); });
  $('select-none').addEventListener('click', () => { state.checked.clear(); renderList(); });

  const list = $('file-list');
  list.addEventListener('click', (e) => {
    const li = e.target.closest('li');
    if (!li || e.target.matches('input')) return;
    select(Number(li.dataset.index));
  });
  list.addEventListener('change', (e) => {
    const li = e.target.closest('li');
    if (!li) return;
    const path = state.files[Number(li.dataset.index)].path;
    if (e.target.checked) state.checked.add(path); else state.checked.delete(path);
    updateCounts();
  });
  list.addEventListener('keydown', (e) => {
    if (e.key === 'ArrowDown') { e.preventDefault(); select(Math.min(state.selected + 1, state.files.length - 1)); }
    else if (e.key === 'ArrowUp') { e.preventDefault(); select(Math.max(state.selected - 1, 0)); }
    else if (e.key === 'Delete' || e.key === 'Backspace') { e.preventDefault(); removeSelected(); }
    else if (e.key === ' ' && state.selected >= 0) {
      e.preventDefault();
      const path = state.files[state.selected].path;
      if (state.checked.has(path)) state.checked.delete(path); else state.checked.add(path);
      const cb = rowAt(state.selected)?.querySelector('input');
      if (cb) cb.checked = state.checked.has(path);
      updateCounts();
    }
  });

  const frame = $('frame');
  frame.addEventListener('pointerdown', onPointerDown);
  frame.addEventListener('pointermove', onPointerMove);
  frame.addEventListener('pointerup', onPointerUp);
  frame.addEventListener('pointercancel', onPointerUp);
  new ResizeObserver(layoutFrame).observe($('stage'));
  $('crop-clear').addEventListener('click', clearCrop);
  $('crop-all').addEventListener('click', cropForAllTicked);

  const presets = $('presets');
  for (const [label, w, h] of PRESETS) {
    const b = document.createElement('button');
    b.className = 'small';
    b.textContent = label;
    b.addEventListener('click', () => {
      $('width').value = w ? String(w) : '';
      $('height').value = h ? String(h) : '';
      $('keep-aspect').checked = true;
      saveSettings();
    });
    presets.append(b);
  }
  $('quality').addEventListener('input', () => { $('quality-out').textContent = $('quality').value; });
  for (const el of document.querySelectorAll('.settings input, .settings select, #crop-ratio')) {
    el.addEventListener('change', saveSettings);
  }
  $('browse').addEventListener('click', async () => {
    const picked = await openDialog({ directory: true, title: 'Choose output folder', defaultPath: $('out-dir').value || undefined });
    if (picked) { $('out-dir').value = picked; saveSettings(); }
  });

  $('process').addEventListener('click', processSelected);
  $('cancel').addEventListener('click', () => {
    $('cancel').disabled = true;
    setStatus('Cancelling: images already in progress will finish…');
    invoke('cancel_processing');
  });
  $('results-close').addEventListener('click', () => $('results').close());

  const filesCard = document.querySelector('.files');
  getCurrentWebview().onDragDropEvent((event) => {
    const { type } = event.payload;
    if (type === 'enter' || type === 'over') filesCard.classList.add('drop-target');
    else filesCard.classList.remove('drop-target');
    if (type === 'drop') addPaths(event.payload.paths);
  });

  getCurrentWindow().onCloseRequested(async (event) => {
    if (!state.running) return;
    const quit = await ask(
      'Images are still being processed. Images already saved are complete; the rest will not be processed.\n\nQuit anyway?',
      { title: 'Imaginify', kind: 'warning', okLabel: 'Quit', cancelLabel: 'Keep working' },
    );
    if (!quit) event.preventDefault();
  });
}

async function init() {
  wire();
  renderList();
  updateCropUi();
  try { $('version').textContent = `v${await window.__TAURI__.app.getVersion()}`; } catch { /* cosmetic */ }
  await loadSettings();
}

init();
