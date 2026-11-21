/* Crystalline renderer — vanilla ES module over the Rust engine JSON-RPC bridge */

const K = window.crystalline;
const $ = (s, r = document) => r.querySelector(s);
const el = (tag, cls, html) => {
  const n = document.createElement(tag);
  if (cls) n.className = cls;
  if (html != null) n.innerHTML = html;
  return n;
};
const esc = (s) => String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
const spacer = (h) => { const d = el('div'); d.style.height = `${h}px`; return d; };

/* ------------------------------- icons ------------------------------- */
const IC = (() => {
  const w = (p) => `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${p}</svg>`;
  return {
    home: w('<path d="M3 10.5 12 3l9 7.5"/><path d="M5 9.5V21h14V9.5"/>'),
    shield: w('<path d="M12 3l7 3v6c0 4.5-3 7.5-7 9-4-1.5-7-4.5-7-9V6l7-3z"/>'),
    shieldCheck: w('<path d="M12 3l7 3v6c0 4.5-3 7.5-7 9-4-1.5-7-4.5-7-9V6l7-3z"/><path d="M9 12l2 2 4-4"/>'),
    archive: w('<rect x="3" y="4" width="18" height="5" rx="1"/><path d="M5 9v11h14V9"/><path d="M10 13h4"/>'),
    cpu: w('<rect x="5" y="5" width="14" height="14" rx="2"/><rect x="9.5" y="9.5" width="5" height="5"/><path d="M9 2v3M15 2v3M9 19v3M15 19v3M2 9h3M2 15h3M19 9h3M19 15h3"/>'),
    gpu: w('<rect x="2" y="6" width="16" height="12" rx="2"/><path d="M18 10h4v4h-4"/><circle cx="9" cy="12" r="2.5"/>'),
    gamepad: w('<path d="M6 8h12a4 4 0 0 1 4 4v2a4 4 0 0 1-6.6 3L14 15.5h-4l-1.4 1.5A4 4 0 0 1 2 14v-2a4 4 0 0 1 4-4z"/><path d="M7 11v3M5.5 12.5h3"/><circle cx="16" cy="12" r=".8"/><circle cx="18" cy="14" r=".8"/>'),
    network: w('<circle cx="12" cy="5" r="2.4"/><circle cx="5" cy="19" r="2.4"/><circle cx="19" cy="19" r="2.4"/><path d="M12 7.4v4.1M12 11.5 6.5 17M12 11.5 17.5 17"/>'),
    bolt: w('<path d="M13 2 4.5 13.5H11l-1 8.5L18.5 10.5H12l1-8.5z"/>'),
    drive: w('<rect x="3" y="5" width="18" height="14" rx="2"/><path d="M3 15h18"/><circle cx="7.5" cy="17" r=".6" fill="currentColor"/><circle cx="11" cy="17" r=".6" fill="currentColor"/>'),
    sparkles: w('<path d="M12 4l1.7 4.6L18 10l-4.3 1.4L12 16l-1.7-4.6L6 10l4.3-1.4L12 4z"/><path d="M18.5 15l.9 2.1 2.1.9-2.1.9-.9 2.1-.9-2.1-2.1-.9 2.1-.9.9-2.1z"/>'),
    window: w('<rect x="3" y="5" width="18" height="15" rx="2"/><path d="M3 9h18"/><circle cx="6" cy="7" r=".6" fill="currentColor"/><circle cx="9" cy="7" r=".6" fill="currentColor"/>'),
    cog: w('<circle cx="12" cy="12" r="3.2"/><path d="M12 2.8v2.7M12 18.5v2.7M4.9 4.9l1.9 1.9M17.2 17.2l1.9 1.9M2.8 12h2.7M18.5 12h2.7M4.9 19.1l1.9-1.9M17.2 6.8l1.9-1.9"/>'),
    clock: w('<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3.5 2"/>'),
    lock: w('<rect x="5" y="10" width="14" height="11" rx="2"/><path d="M8 10V7a4 4 0 0 1 8 0v3"/>'),
    layers: w('<path d="M12 3 3 8l9 5 9-5-9-5z"/><path d="M3 13l9 5 9-5"/><path d="M3 18l9 5 9-5"/>'),
    search: w('<circle cx="11" cy="11" r="7"/><path d="M21 21l-4.3-4.3"/>'),
    info: w('<circle cx="12" cy="12" r="9"/><path d="M12 11v5"/><circle cx="12" cy="8" r=".7" fill="currentColor"/>'),
    refresh: w('<path d="M20 12a8 8 0 1 1-2.3-5.6"/><path d="M20 3v4h-4"/>'),
    folder: w('<path d="M3 7a2 2 0 0 1 2-2h4l2 2.5h8a2 2 0 0 1 2 2V17a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7z"/>'),
    restore: w('<path d="M3 12a9 9 0 1 0 3-6.7"/><path d="M3 4v5h5"/><path d="M12 8v4l3 2"/>'),
    alert: w('<path d="M12 3 1.8 20.2h20.4L12 3z"/><path d="M12 10v4.5"/><circle cx="12" cy="17.4" r=".7" fill="currentColor"/>'),
    check: w('<path d="M4 12.5 9.5 18 20 6.5"/>'),
    x: w('<path d="M5 5l14 14M19 5 5 19"/>'),
    play: w('<path d="M7 4.5 19 12 7 19.5V4.5z"/>'),
    undo: w('<path d="M9 14 4 9l5-5"/><path d="M4 9h9a7 7 0 0 1 0 14h-2"/>'),
    wrench: w('<path d="M14.5 6.5a4.5 4.5 0 0 0-6 6L3 18l3 3 5.5-5.5a4.5 4.5 0 0 0 6-6L14 13l-3-3 3.5-3.5z"/>'),
    shieldOff: w('<path d="M12 3l7 3v6c0 4.5-3 7.5-7 9-4-1.5-7-4.5-7-9V6l7-3z"/><path d="M9 9.5 15 15.5M15 9.5 9 15.5"/>'),
    explorer: w('<rect x="3" y="5" width="18" height="15" rx="2"/><path d="M3 9h18"/><path d="M12 12l-2.5 4M12 12l2.5 4M12 12v5"/>'),
  };
})();

/* ------------------------------- state ------------------------------- */
const S = {
  catalog: null,
  hw: null,
  scan: null,
  snapshots: [],
  view: { kind: 'home' },       // home | safety | backups | cat | preset | search
  query: '',
  subFilter: 'All',
  busy: new Set(),              // tweak ids currently applying/reverting
};
const scanOf = (id) => S.scan?.results?.[id];
const catOf = (id) => S.catalog.categories.find((c) => c.id === id);
const tweakOf = (id) => S.catalog.tweaks.find((t) => t.id === id);

const STATUS = {
  APPLIED: { label: 'On', cls: 's-on', on: true },
  NEAR_MATCH: { label: 'Custom variant', cls: 's-near', on: true, near: true },
  PARTIAL_DRIFT: { label: 'Partially set', cls: 's-drift', on: true, near: true },
  HAZARDOUS_CUSTOM: { label: 'Unsafe value', cls: 's-hazard', on: true, near: true },
  DEFAULT: { label: 'Off', cls: '', on: false },
  INCOMPATIBLE_HARDWARE: { label: 'Not for this PC', cls: 's-na', on: false, dead: true },
  NOT_INSTALLED: { label: 'Not present', cls: 's-na', on: false, dead: true },
};
const RISK_LABEL = { safe: 'SAFE', moderate: 'MOD', caution: 'CAUTION', expert: 'EXPERT' };
const IMPACT = { safe: 1, moderate: 2, caution: 3, expert: 4 };

/* ------------------------------- toasts ------------------------------- */
function toast(kind, title, msg, action) {
  const root = $('#toasts');
  const t = el('div', `toast t-${kind}`);
  t.innerHTML = `<span class="t-dot"></span><div class="t-body"><div class="t-title">${esc(title)}</div>${msg ? `<div class="t-msg">${esc(msg)}</div>` : ''}</div>`;
  let killTimer = null;
  if (action) {
    const b = el('button', 't-act', esc(action.label));
    b.onclick = () => { action.fn(); dismiss(); };
    t.appendChild(b);
    killTimer = setTimeout(dismiss, 12000);
  } else {
    killTimer = setTimeout(dismiss, 6500);
  }
  function dismiss() {
    clearTimeout(killTimer);
    t.classList.add('out');
    setTimeout(() => t.remove(), 220);
  }
  root.appendChild(t);
  while (root.children.length > 5) root.firstChild.remove();
}

/* ------------------------------- modal ------------------------------- */
function openModal({ title, body, foot }) {
  const root = $('#modal-root');
  root.innerHTML = '';
  const veil = el('div', 'modal-veil');
  const m = el('div', 'modal');
  m.setAttribute('role', 'dialog');
  m.setAttribute('aria-modal', 'true');
  const head = el('div', 'modal-head');
  head.appendChild(el('div', 'modal-title', title));
  const x = el('button', 'modal-x', IC.x);
  x.setAttribute('aria-label', 'Close dialog');
  x.onclick = close;
  head.appendChild(x);
  m.appendChild(head);
  const bd = el('div', 'modal-body');
  if (typeof body === 'string') bd.innerHTML = body; else bd.appendChild(body);
  m.appendChild(bd);
  if (foot) {
    const f = el('div', 'modal-foot');
    foot.forEach((a) => {
      const b = el('button', `btn ${a.kind || ''}`, esc(a.label));
      b.onclick = () => a.fn(close, b);
      f.appendChild(b);
    });
    m.appendChild(f);
  }
  veil.appendChild(m);
  veil.addEventListener('mousedown', (e) => { if (e.target === veil) close(); });
  root.appendChild(veil);
  const onKey = (e) => { if (e.key === 'Escape') close(); };
  document.addEventListener('keydown', onKey, { once: false });
  function close() {
    document.removeEventListener('keydown', onKey);
    root.innerHTML = '';
  }
  return close;
}

function confirmModal({ title, body, confirmLabel, danger }) {
  return new Promise((resolve) => {
    const close = openModal({
      title: esc(title),
      body,
      foot: [
        { label: 'Cancel', fn: (c) => { c(); resolve(false); } },
        {
          label: confirmLabel,
          kind: danger ? 'btn-danger' : 'btn-primary',
          fn: (c) => { c(); resolve(true); },
        },
      ],
    });
  });
}

/* ------------------------------- sidebar ------------------------------- */
const NAV_ICONS = {
  home: IC.home, safety: IC.shield, backups: IC.archive,
};

function navCount(catId) {
  const list = S.catalog.tweaks.filter((t) => t.category === catId);
  const on = list.filter((t) => STATUS[scanOf(t.id)?.status || 'DEFAULT']?.on).length;
  return { on, total: list.length };
}

function renderNav() {
  const host = $('#nav-scroll');
  host.innerHTML = '';

  const mkGroup = (label) => {
    const g = el('div', 'nav-group');
    g.appendChild(el('div', 'nav-label', esc(label)));
    host.appendChild(g);
    return g;
  };
  const mkItem = (parent, { icon, label, count, view, active }) => {
    const b = el('button', `nav-item${active ? ' active' : ''}`);
    b.innerHTML = `${icon}<span>${esc(label)}</span>${count ? `<span class="nav-count ${count.cls || ''}">${count.txt}</span>` : ''}`;
    b.onclick = () => go(view);
    parent.appendChild(b);
  };
  const v = S.view;

  const g1 = mkGroup('General');
  mkItem(g1, { icon: IC.home, label: 'Home', view: { kind: 'home' }, active: v.kind === 'home' });
  const hazardN = S.scan?.badTweaks?.length || 0;
  mkItem(g1, {
    icon: IC.shield, label: 'Safety center',
    count: hazardN ? { txt: String(hazardN), cls: '' } : null,
    view: { kind: 'safety' }, active: v.kind === 'safety',
  });
  mkItem(g1, {
    icon: IC.archive, label: 'Backups',
    count: S.snapshots.length ? { txt: String(S.snapshots.length) } : null,
    view: { kind: 'backups' }, active: v.kind === 'backups',
  });

  const g2 = mkGroup('Tweaks');
  for (const c of S.catalog.categories) {
    const n = navCount(c.id);
    mkItem(g2, {
      icon: IC[c.icon] || IC.layers,
      label: c.shortName,
      count: { txt: `${n.on}/${n.total}`, cls: n.on ? 'some' : '' },
      view: { kind: 'cat', id: c.id },
      active: v.kind === 'cat' && v.id === c.id,
    });
  }

  const g3 = mkGroup('Presets');
  for (const p of S.catalog.presets) {
    mkItem(g3, {
      icon: IC.layers, label: p.name.split(' ').slice(0, 2).join(' '),
      count: { txt: String(p.tweakIds.length) },
      view: { kind: 'preset', id: p.id },
      active: v.kind === 'preset' && v.id === p.id,
    });
  }

  const foot = $('#nav-foot');
  const ms = S.scan ? `${Math.round(S.scan.scanDurationMs)}ms` : '—';
  foot.innerHTML = `<span class="dot${S.hw?.isElevated ? '' : ' off'}"></span><span>${S.hw?.isElevated ? 'elevated' : 'standard rights'} · engine ${ms}</span>`;

  const tb = $('#tb-status');
  if (S.hw && !S.hw.isElevated) {
    tb.hidden = false;
    tb.textContent = 'Not administrator — click to relaunch elevated';
    tb.style.cursor = 'pointer';
    tb.onclick = async () => {
      const ok = await confirmModal({
        title: 'Relaunch as administrator?',
        body: `<div class="m-note">Machine-level tweaks (registry under HKLM, services, power plans, netsh) need elevation. Crystalline will close and relaunch with a UAC prompt.</div>`,
        confirmLabel: 'Relaunch elevated',
      });
      if (ok) K.relaunchAdmin();
    };
  } else {
    tb.hidden = true;
    tb.onclick = null;
  }
}

/* ------------------------------- view: cards ------------------------------- */
function stateFor(t) {
  const st = scanOf(t.id)?.status || 'DEFAULT';
  return STATUS[st] || STATUS.DEFAULT;
}

function impactBars(risk) {
  const n = IMPACT[risk] || 1;
  return `<span class="tc-impact i-${n}" title="System impact: ${esc(risk)}"><i></i><i></i><i></i></span>`;
}

function tweakCard(t) {
  const r = scanOf(t.id);
  const st = stateFor(t);
  const blockedAdmin = t.requiresAdmin && !S.hw?.isElevated;
  const disabled = st.dead || blockedAdmin || S.busy.has(t.id);
  const reason = st.dead ? (r?.statusReason || 'Not applicable to this machine')
    : blockedAdmin ? 'Requires administrator — relaunch elevated' : '';

  const card = el('div', `tweak-card${st.on && !st.near ? ' is-on' : ''}${st.dead ? ' is-dim' : ''}${S.busy.has(t.id) ? ' is-busy' : ''}`);

  const head = el('div', 'tc-head');
  head.innerHTML = `
    <span class="tc-status ${st.cls}"><span class="sdot"></span>${esc(st.label)}</span>
    <span class="tc-risk r-${esc(t.risk)}">${RISK_LABEL[t.risk] || t.risk.toUpperCase()}</span>`;
  card.appendChild(head);

  card.appendChild(el('div', 'tc-title', esc(t.title)));
  card.appendChild(el('div', 'tc-desc', esc(t.description)));

  const foot = el('div', 'tc-foot');
  foot.appendChild(el('span', '', impactBars(t.risk)));
  if (reason) foot.appendChild(el('span', 'tc-flag', esc(reason.slice(0, 44))));
  else {
    const flags = [];
    if (t.rebootRequired) flags.push('reboot');
    if (t.explorerRestart) flags.push('shell restart');
    if (r?.hasSnapshotBackup) flags.push('backed up');
    if (flags.length) foot.appendChild(el('span', 'tc-flag', esc(flags.join(' · '))));
  }
  const info = el('button', 'tc-info', 'i');
  info.title = 'Details';
  info.setAttribute('aria-label', `Details for ${t.title}`);
  info.onclick = () => openDetail(t);
  foot.appendChild(info);

  const sw = el('button', `switch${st.near ? ' sw-near' : ''}`);
  sw.setAttribute('role', 'switch');
  sw.setAttribute('aria-checked', String(st.on));
  sw.setAttribute('aria-label', `${st.on ? 'Turn off' : 'Turn on'} ${t.title}`);
  sw.disabled = disabled;
  sw.title = reason || (st.on ? 'Turn off (restore previous state)' : 'Turn on');
  sw.onclick = () => toggleTweak(t, !st.on, sw);
  foot.appendChild(sw);

  card.appendChild(foot);
  return card;
}

/* ------------------------------- detail modal ------------------------------- */
function openDetail(t) {
  const r = scanOf(t.id);
  const body = el('div');
  body.innerHTML = `
    <div class="m-note">${esc(t.description)}</div>
    ${r?.statusReason ? `<div class="m-warnline">${IC.info}<span>${esc(r.statusReason)}</span></div>` : ''}
    ${(r?.eligibilityWarnings || []).map((w) => `<div class="m-warnline">${IC.alert}<span>${esc(w)}</span></div>`).join('')}
    <div class="dl-sec"><div class="dl-k">WHAT IT CHANGES</div><div class="dl-v">${esc(t.technicalNotes)}</div></div>
    <div class="dl-sec"><div class="dl-k">SAFETY</div><div class="dl-v">${esc(t.safetyNotes)}</div></div>
    <div class="dl-sec"><div class="dl-k">OPERATIONS & LIVE VALUES</div>
      ${(r?.opDetails || []).map((o) => `
        <div class="op-line"><span class="op-kind">${esc(o.status)}</span><span>${esc(o.targetLabel)}</span><span class="cur">${esc(o.currentValue)}</span>→<span class="ok">${esc(o.expectedApply)}</span></div>
      `).join('') || `<div class="dl-v">${t.ops.length} operation(s)</div>`}
    </div>
    <div class="tag-row">
      ${t.tags.map((x) => `<span class="tag">${esc(x)}</span>`).join('')}
      ${t.rebootRequired ? '<span class="tag">reboot required</span>' : ''}
      ${t.explorerRestart ? '<span class="tag">restarts shell</span>' : ''}
      ${t.requiresAdmin ? '<span class="tag">admin</span>' : ''}
      ${r?.hasSnapshotBackup ? '<span class="tag">snapshot backed</span>' : ''}
      ${(r?.activeConflicts || []).map((x) => `<span class="tag">conflicts: ${esc(x)}</span>`).join('')}
    </div>`;
  openModal({ title: esc(t.title), body });
}

/* ------------------------------- apply / revert ------------------------------- */
async function toggleTweak(t, turnOn, sw, opts = {}) {
  const st = stateFor(t);
  const needsConfirm = !opts.skipConfirm && (t.risk === 'caution' || t.risk === 'expert' || st.near || !turnOn);
  if (needsConfirm) {
    const extra = st.near && turnOn
      ? `<div class="m-warnline">${IC.alert}<span>A custom value is already set here and will be replaced by the recommended one. The current value is snapshotted first.</span></div>`
      : '';
    const ok = await confirmModal({
      title: `${turnOn ? 'Turn on' : 'Turn off'} "${t.title}"?`,
      body: `<div class="m-note">${esc(t.safetyNotes)}</div>${extra}
             ${t.rebootRequired ? `<div class="m-warnline">${IC.alert}<span>Takes effect after a restart.</span></div>` : ''}
             ${t.explorerRestart ? `<div class="m-warnline">${IC.alert}<span>Windows Explorer will restart — taskbar flashes briefly.</span></div>` : ''}`,
      confirmLabel: turnOn ? 'Turn on' : 'Turn off',
      danger: t.risk === 'expert',
    });
    if (!ok) return;
  }

  S.busy.add(t.id);
  const card = sw.closest('.tweak-card');
  if (card) card.classList.add('is-busy');
  sw.innerHTML = '<span class="sw-spin"></span>';

  try {
    const res = turnOn
      ? await K.rpc('apply', { tweakIds: [t.id], allowOverride: true })
      : await K.rpc('revert', { tweakIds: [t.id] });
    const rep = res.report;
    if (res.scan) S.scan = res.scan;
    if (res.snapshots) S.snapshots = res.snapshots;

    if (rep.failed?.length) {
      toast('err', turnOn ? 'Could not turn on' : 'Could not turn off', rep.failed.map(([, e]) => e).join(' ').slice(0, 220));
    } else {
      const undoAction = { label: 'Undo', fn: () => toggleTweak(t, !turnOn, document.createElement('button'), { skipConfirm: true }) };
      toast('ok', turnOn ? 'Turned on' : 'Turned off', `${t.title}${rep.snapshotId ? ' — snapshot ' + rep.snapshotId : ''}`, undoAction);
    }
    rep.ucpdWarnings?.forEach((w) => toast('warn', 'Windows blocked a write', w, null));
    if (rep.explorerRestartNeeded) {
      toast('warn', 'Explorer restart pending', 'Restart the shell to finish.', { label: 'Restart now', fn: () => K.rpc('restart_explorer') });
    }
    if (rep.rebootNeeded) toast('warn', 'Restart needed', 'Restart Windows to finalize this change.');
  } catch (e) {
    toast('err', 'Engine error', String(e.message || e).slice(0, 240));
  } finally {
    S.busy.delete(t.id);
    renderNav();
    rerender();
  }
}

/* ------------------------------- dry run ------------------------------- */
async function openDryRun(ids, mode = 'apply') {
  const names = { apply: 'Preview changes', revert: 'Preview restore' };
  const body = el('div');
  body.innerHTML = `<div class="m-note">Computing a live before/after diff for ${ids.length} tweak(s)…</div>`;
  const close = openModal({ title: names[mode] || 'Preview', body });

  let diff;
  try {
    diff = await K.rpc('dry_run', { tweakIds: ids, mode });
  } catch (e) {
    body.innerHTML = `<div class="m-note">Dry run failed: ${esc(e.message || e)}</div>`;
    return;
  }
  const open = diff.filter((d) => !d.blocked);
  body.innerHTML = `
    <div class="diff-head"><span>Tweak</span><span>Current</span><span>After</span></div>
    ${diff.map((d) => `
      <div class="diff-row${d.blocked ? ' d-blocked' : ''}">
        <span class="d-name">${esc(d.tweakTitle)}</span>
        <span class="d-before">${esc(d.before)}</span>
        <span class="d-after">${esc(d.after)}</span>
        <span class="d-target">${esc(d.target)}</span>
        ${d.blocked ? `<span class="d-block">Blocked — ${esc(d.blockReason)}</span>` : ''}
        ${(d.warnings || []).map((w) => `<span class="d-block" style="color:var(--warn)">${esc(w)}</span>`).join('')}
      </div>`).join('')}`;
  const m = $('.modal-foot', body.closest('.modal'));
  if (m) m.remove();
  const foot = el('div', 'modal-foot');
  const cancel = el('button', 'btn', 'Cancel');
  cancel.onclick = close;
  foot.appendChild(cancel);
  const go = el('button', `btn ${mode === 'apply' ? 'btn-primary' : 'btn-danger'}`,
    mode === 'apply' ? `Apply ${open.length} change${open.length === 1 ? '' : 's'}` : `Restore ${open.length}`);
  go.disabled = !open.length;
  go.onclick = async () => {
    close();
    await applyBatch(open.map((d) => d.tweakId), mode);
  };
  foot.appendChild(go);
  $('.modal').appendChild(foot);
}

async function applyBatch(ids, mode) {
  try {
    const res = mode === 'apply'
      ? await K.rpc('apply', { tweakIds: ids, allowOverride: true })
      : await K.rpc('revert', { tweakIds: ids });
    const rep = res.report;
    if (res.scan) S.scan = res.scan;
    if (res.snapshots) S.snapshots = res.snapshots;
    const okN = rep.succeeded?.length || 0;
    const failN = rep.failed?.length || 0;
    if (okN) toast('ok', mode === 'apply' ? 'Changes applied' : 'State restored', `${okN} tweak(s)${rep.snapshotId ? ` — snapshot ${rep.snapshotId}` : ''}${failN ? `, ${failN} blocked/failed` : ''}`);
    if (failN) toast('err', `${failN} could not be changed`, rep.failed.map(([id, e]) => `${id}: ${e}`).join(' · ').slice(0, 240));
    rep.ucpdWarnings?.forEach((w) => toast('warn', 'Windows blocked a write', w));
    if (rep.explorerRestartNeeded) toast('warn', 'Explorer restart pending', 'Restart the shell to finish.', { label: 'Restart now', fn: () => K.rpc('restart_explorer') });
    if (rep.rebootNeeded) toast('warn', 'Restart needed', 'Restart Windows to finalize the changes.');
  } catch (e) {
    toast('err', 'Engine error', String(e.message || e).slice(0, 240));
  } finally {
    renderNav();
    rerender();
  }
}

/* ------------------------------- views ------------------------------- */
function statsFor(catId) {
  const list = catId ? S.catalog.tweaks.filter((t) => t.category === catId) : S.catalog.tweaks;
  const live = list.filter((t) => {
    const st = scanOf(t.id)?.status;
    return st !== 'INCOMPATIBLE_HARDWARE' && st !== 'NOT_INSTALLED';
  });
  const on = live.filter((t) => STATUS[scanOf(t.id)?.status || 'DEFAULT']?.on).length;
  return { on, total: live.length, pct: live.length ? Math.round((on / live.length) * 100) : 0 };
}

function ringSvg(pct) {
  const r = 5.5, c = 2 * Math.PI * r;
  return `<svg class="ring" viewBox="0 0 14 14"><circle cx="7" cy="7" r="${r}" fill="none" stroke="var(--line-2)" stroke-width="2"/><circle cx="7" cy="7" r="${r}" fill="none" stroke="var(--accent)" stroke-width="2" stroke-dasharray="${(pct / 100 * c).toFixed(1)} ${c.toFixed(1)}" stroke-linecap="round" transform="rotate(-90 7 7)"/></svg>`;
}

function pageHead({ title, sub, catId }) {
  const s = statsFor(catId);
  const head = el('div', 'page-head');
  head.innerHTML = `
    <div><div class="page-title">${esc(title)}</div>${sub ? `<div class="page-sub">${esc(sub)}</div>` : ''}</div>
    <div class="protect-pill">${ringSvg(s.pct)}<span><span class="pct">${s.pct}%</span> on — ${s.on} of ${s.total} protections</span></div>`;
  return head;
}

function searchBar() {
  const wrap = el('div', 'search-box');
  wrap.innerHTML = `${IC.search}<input type="text" placeholder="Search 376 tweaks…" spellcheck="false"/><kbd>Ctrl F</kbd>`;
  const input = $('input', wrap);
  input.value = S.query;
  input.addEventListener('input', () => {
    S.query = input.value;
    if (S.query.trim()) go({ kind: 'search' }, { keepQuery: true });
    else if (S.view.kind === 'search') go({ kind: 'home' });
    else rerender();
  });
  return wrap;
}

/* ----- home ----- */
function renderHome(root) {
  const hw = S.hw || {};
  const p = el('div', 'page');
  p.appendChild(pageHead({ title: 'System overview', sub: `${hw.osCaption || 'Windows'} · Build ${hw.buildNumber || '—'}${hw.ucpdActive ? ' · UCPD active' : ''}` }));

  if ((S.scan?.badTweaks?.length || 0) > 0) {
    const b = el('div', 'banner b-danger');
    b.innerHTML = `${IC.alert}<span class="grow"><strong>${S.scan.badTweaks.length} unsafe setting${S.scan.badTweaks.length === 1 ? '' : 's'} found</strong> — values known to damage stability or performance are active on this machine.</span>`;
    const fix = el('button', 'btn', 'Review fixes');
    fix.onclick = () => go({ kind: 'safety' });
    b.appendChild(fix);
    p.appendChild(b);
  }

  const strip = el('div', 'stat-strip');
  const chips = [
    ['Processor', hw.cpuName || '—'],
    ['Memory', `${hw.totalRamGb || '—'} GB · ${hw.logicalCores || '—'} threads`],
    ['Graphics', (hw.gpuNames || []).join(' + ') || '—'],
    ['Storage', hw.nvmePresent ? 'NVMe SSD' : hw.primaryDriveSsd ? 'SSD' : 'HDD / unknown'],
    ['Session', hw.isElevated ? 'Administrator' : 'Standard user', hw.isElevated],
    ['System restore', hw.systemRestoreEnabled ? 'Enabled' : 'Will enable on demand', hw.systemRestoreEnabled],
  ];
  for (const [k, v, ok] of chips) {
    strip.appendChild(el('div', 'stat-chip', `<div class="k">${esc(k)}</div><div class="v${ok === true ? ' ok' : ok === false ? ' warn' : ''}">${esc(String(v))}</div>`));
  }
  p.appendChild(strip);

  const acts = el('div', 'toolbar');
  const mk = (icon, label, fn, primary) => {
    const b = el('button', `btn${primary ? ' btn-primary' : ''}`, `${icon}${esc(label)}`);
    b.onclick = fn;
    acts.appendChild(b);
    return b;
  };
  mk(IC.refresh, 'Rescan system', async () => {
    try {
      S.scan = await K.rpc('scan');
      toast('ok', 'Scan complete', `${S.scan.totalTweaks} targets in ${Math.round(S.scan.scanDurationMs)}ms`);
      renderNav(); rerender();
    } catch (e) { toast('err', 'Scan failed', String(e.message || e)); }
  });
  mk(IC.restore, 'Create restore point', async () => {
    const ok = await confirmModal({
      title: 'Create a Windows restore point?',
      body: `<div class="m-note">Snapshots a system-level checkpoint via Windows System Restore. Takes about a minute. If System Restore is off, Crystalline enables it first.</div>`,
      confirmLabel: 'Create restore point',
    });
    if (!ok) return;
    try {
      const r = await K.rpc('create_restore_point', { label: 'Crystalline Manual' });
      toast('ok', 'Restore point created', r.message);
    } catch (e) { toast('err', 'Restore point failed', String(e.message || e).slice(0, 300)); }
  });
  mk(IC.explorer, 'Restart Explorer', async () => {
    const ok = await confirmModal({ title: 'Restart Windows Explorer?', body: `<div class="m-note">The taskbar and file windows close and reopen. Unsaved File Explorer paths are lost.</div>`, confirmLabel: 'Restart Explorer' });
    if (ok) K.rpc('restart_explorer');
  });
  mk(IC.folder, 'Open backups folder', () => K.openBackupsFolder());
  p.appendChild(acts);

  const blk = el('div', 'block');
  blk.appendChild(el('div', 'block-title', `Coverage by category <span class="cnt">${S.catalog.tweaks.length} tweaks</span>`));
  const panel = el('div', 'panel');
  panel.style.padding = '6px';
  for (const c of S.catalog.categories) {
    const n = navCount(c.id);
    const pct = n.total ? Math.round((n.on / n.total) * 100) : 0;
    const row = el('button', 'cat-row');
    row.innerHTML = `${IC[c.icon] || IC.layers}<span class="nm">${esc(c.shortName)}</span><span class="prog"><i style="width:${pct}%"></i></span><span class="cnt">${n.on}/${n.total}</span>`;
    row.onclick = () => go({ kind: 'cat', id: c.id });
    panel.appendChild(row);
  }
  blk.appendChild(panel);
  p.appendChild(blk);

  if (S.snapshots.length) {
    const b2 = el('div', 'block');
    b2.appendChild(el('div', 'block-title', `Recent snapshots <span class="cnt">${S.snapshots.length} total</span>`));
    S.snapshots.slice(0, 3).forEach((s) => b2.appendChild(snapRow(s)));
    p.appendChild(b2);
  }
  root.appendChild(p);
}

function snapRow(s) {
  const row = el('div', 'snap-row');
  row.innerHTML = `<div class="grow"><div class="s-id">${esc(s.snapshotId)}</div><div class="s-meta">${esc(s.createdAt)} · ${s.tweakIds.length} tweak(s) · sha ${esc(String(s.sha256 || '').slice(0, 12))}…</div></div>`;
  const b = el('button', 'btn', `${IC.restore}Restore`);
  b.onclick = async () => {
    const ok = await confirmModal({
      title: `Restore snapshot ${s.snapshotId}?`,
      body: `<div class="m-note">Every value in this snapshot returns to its captured pre-change state — including values other tools changed afterwards.</div>`,
      confirmLabel: 'Restore snapshot',
    });
    if (!ok) return;
    try {
      const res = await K.rpc('restore_snapshot', { snapshotId: s.snapshotId });
      if (res.scan) S.scan = res.scan;
      toast('ok', 'Snapshot restored', `${res.report?.succeeded?.length || 0} value(s) restored.`);
      renderNav(); rerender();
    } catch (e) { toast('err', 'Restore failed', String(e.message || e).slice(0, 240)); }
  };
  row.appendChild(b);
  return row;
}

/* ----- category ----- */
function renderCategory(root, catId) {
  const cat = catOf(catId);
  const list = S.catalog.tweaks.filter((t) => t.category === catId);
  const p = el('div', 'page');
  p.appendChild(pageHead({ title: cat.name, sub: cat.description, catId }));

  const bar = el('div', 'toolbar');
  bar.appendChild(searchBar());
  p.appendChild(bar);

  const subs = ['All', ...cat.subcategories];
  const chips = el('div', 'chips');
  for (const s of subs) {
    const c = el('button', `chip${S.subFilter === s ? ' on' : ''}`, esc(s));
    c.onclick = () => { S.subFilter = s; rerender(); };
    chips.appendChild(c);
  }
  p.appendChild(chips);
  p.appendChild(spacer(14));

  const shown = S.subFilter === 'All' ? list : list.filter((t) => t.subcategory === S.subFilter);
  const grid = el('div', 'card-grid');
  shown.forEach((t) => grid.appendChild(tweakCard(t)));
  if (!shown.length) grid.appendChild(el('div', 'empty', `<div class="e-title">Nothing in this group</div>`));
  p.appendChild(grid);
  root.appendChild(p);
}

/* ----- search ----- */
function renderSearch(root) {
  const q = S.query.trim().toLowerCase();
  const p = el('div', 'page');
  p.appendChild(pageHead({ title: `Search`, sub: 'Matches across titles, descriptions, tags and targets.' }));
  const bar = el('div', 'toolbar');
  bar.appendChild(searchBar());
  p.appendChild(bar);

  if (!q) {
    p.appendChild(el('div', 'empty', `<div class="e-title">Type to search 376 tweaks</div>`));
    root.appendChild(p);
    return;
  }
  const hits = S.catalog.tweaks.filter((t) =>
    [t.title, t.description, t.subcategory, t.id, ...(t.tags || []), ...(t.ops || []).map((o) => o.targetLabel || o.subkey || o.serviceName || o.taskPath || '')]
      .join(' ').toLowerCase().includes(q));
  if (!hits.length) {
    const e = el('div', 'empty', `<div class="e-title">No results for “${esc(S.query)}”</div><div class="e-sub">Try a service name, registry value or feature.</div>`);
    const b = el('button', 'btn', 'Clear search');
    b.onclick = () => { S.query = ''; go({ kind: 'home' }); };
    e.appendChild(b);
    p.appendChild(e);
    root.appendChild(p);
    return;
  }
  const byCat = {};
  hits.forEach((t) => { (byCat[t.category] = byCat[t.category] || []).push(t); });
  for (const c of S.catalog.categories) {
    const arr = byCat[c.id];
    if (!arr?.length) continue;
    p.appendChild(el('div', 'group-head', `${IC[c.icon] || IC.layers}${esc(c.shortName)} <span class="cnt">${arr.length}</span>`));
    const grid = el('div', 'card-grid');
    arr.forEach((t) => grid.appendChild(tweakCard(t)));
    p.appendChild(grid);
  }
  root.appendChild(p);
}

/* ----- preset ----- */
function renderPreset(root, id) {
  const pre = S.catalog.presets.find((x) => x.id === id);
  const p = el('div', 'page');
  p.appendChild(pageHead({ title: pre.name, sub: pre.subtitle }));

  const panel = el('div', 'panel panel-row');
  const pending = pre.tweakIds.filter((tid) => !STATUS[scanOf(tid)?.status || 'DEFAULT'].on);
  panel.innerHTML = `<div class="grow"><div class="panel-title">${pre.tweakIds.length} tweaks · ${pending.length} not yet on</div><div class="panel-sub">Preview shows a live before/after diff before anything is written.</div></div>`;
  const prev = el('button', 'btn', `${IC.search}Preview changes`);
  prev.onclick = () => openDryRun(pre.tweakIds, 'apply');
  const ap = el('button', 'btn btn-primary', `${IC.play}Apply preset`);
  ap.disabled = !pending.length;
  ap.onclick = () => openDryRun(pre.tweakIds, 'apply');
  panel.appendChild(prev);
  panel.appendChild(ap);
  p.appendChild(panel);
  p.appendChild(spacer(16));

  const grid = el('div', 'card-grid');
  pre.tweakIds.forEach((tid) => {
    const t = tweakOf(tid);
    if (t) grid.appendChild(tweakCard(t));
  });
  p.appendChild(grid);
  root.appendChild(p);
}

/* ----- safety ----- */
function renderSafety(root) {
  const hw = S.hw || {};
  const p = el('div', 'page');
  p.appendChild(pageHead({ title: 'Safety center', sub: 'Checkpoints, snapshots, hazard repair and global rollback.' }));

  const sr = el('div', 'panel panel-row');
  sr.innerHTML = `
    <div class="grow"><div class="panel-title">Windows System Restore</div>
    <div class="panel-sub">${hw.systemRestoreEnabled
      ? 'Enabled on the system drive. A checkpoint captures registry and system files.'
      : 'Currently off. Crystalline will enable it and start the shadow-copy services before creating a point.'}</div></div>`;
  const mk = el('button', 'btn btn-primary', `${IC.restore}Create restore point`);
  mk.onclick = async (e) => {
    const btn = e.currentTarget;
    btn.disabled = true;
    btn.innerHTML = '<span class="b-spin"></span>Working…';
    try {
      const r = await K.rpc('create_restore_point', { label: 'Crystalline Manual' });
      toast('ok', 'Restore point created', r.message);
    } catch (e2) {
      toast('err', 'Restore point failed', String(e2.message || e2).slice(0, 300));
    } finally {
      btn.disabled = false;
      btn.innerHTML = `${IC.restore}Create restore point`;
    }
  };
  sr.appendChild(mk);
  p.appendChild(sr);

  if (hw.ucpdActive) {
    const b = el('div', 'banner b-warn');
    b.style.marginTop = '14px';
    b.innerHTML = `${IC.alert}<span class="grow"><strong>UCPD is active</strong> — Windows 11's User Choice Protection Driver silently reverts some file-association and taskbar writes. Affected tweaks are flagged after apply.</span>`;
    p.appendChild(b);
  }

  const bads = S.scan?.badTweaks || [];
  const blk = el('div', 'block');
  blk.style.marginTop = '22px';
  blk.appendChild(el('div', 'block-title', `Unsafe settings detected <span class="cnt">${bads.length}</span>`));
  if (!bads.length) {
    blk.appendChild(el('div', 'panel', `<div class="panel-sub">No known toxic values found. This check covers forced HPET, disabled TRIM, disabled TCP auto-tuning, removed CPU mitigations, and other damage left by aggressive tweak packs.</div>`));
  } else {
    bads.forEach((f) => {
      const row = el('div', 'finding');
      row.innerHTML = `
        <span class="sev sev-${esc(f.severity)}">${esc(f.severity.toUpperCase())}</span>
        <div class="f-body">
          <div class="f-title">${esc(f.title)}</div>
          <div class="f-why">${esc(f.hazardExplanation)}</div>
          <div class="f-path">${esc(f.targetPath)}</div>
          <div class="f-cur">Now <b>${esc(f.currentValue)}</b> → safe <b>${esc(f.safeValue)}</b></div>
        </div>`;
      const hb = el('button', 'btn', `${IC.wrench}Fix`);
      hb.onclick = async () => {
        hb.disabled = true;
        hb.innerHTML = '<span class="b-spin"></span>';
        try {
          const r = await K.rpc('heal_bad_tweak', { findingId: f.id });
          if (r.scan) S.scan = r.scan;
          toast('ok', 'Repaired', r.message);
          renderNav(); rerender();
        } catch (e) {
          toast('err', 'Repair failed', String(e.message || e).slice(0, 240));
          hb.disabled = false; hb.innerHTML = `${IC.wrench}Fix`;
        }
      };
      row.appendChild(hb);
      blk.appendChild(row);
    });
  }
  p.appendChild(blk);

  const revertable = S.catalog.tweaks.filter((t) => {
    const r = scanOf(t.id);
    return r && STATUS[r.status]?.on && r.hasSnapshotBackup;
  });
  const dz = el('div', 'panel panel-row');
  dz.style.borderColor = 'rgba(239,106,106,.3)';
  dz.innerHTML = `<div class="grow"><div class="panel-title">Revert everything Crystalline changed</div><div class="panel-sub">${revertable.length} tweak(s) have snapshots and will return to their exact captured values.</div></div>`;
  const dzBtn = el('button', 'btn btn-danger', `${IC.undo}Revert ${revertable.length} tweaks`);
  dzBtn.disabled = !revertable.length;
  dzBtn.onclick = async () => {
    const ok = await confirmModal({
      title: `Revert ${revertable.length} tweaks?`,
      body: `<div class="m-note">All ${revertable.length} changes made through Crystalline return to their captured pre-change state. This cannot be undone as one step — snapshots remain on disk in the backups folder.</div>`,
      confirmLabel: `Revert ${revertable.length} tweaks`,
      danger: true,
    });
    if (ok) openDryRun(revertable.map((t) => t.id), 'revert');
  };
  dz.appendChild(dzBtn);
  p.appendChild(dz);
  root.appendChild(p);
}

/* ----- backups ----- */
function renderBackups(root) {
  const p = el('div', 'page');
  p.appendChild(pageHead({ title: 'Backups & snapshots', sub: 'Point-in-time value captures with SHA-256 integrity plus offline .reg/.cmd recovery files.' }));

  const bar = el('div', 'toolbar');
  const open = el('button', 'btn', `${IC.folder}Open backups folder`);
  open.onclick = () => K.openBackupsFolder();
  bar.appendChild(open);
  p.appendChild(bar);

  if (!S.snapshots.length) {
    const e = el('div', 'empty', `<div class="e-title">No snapshots yet</div><div class="e-sub">A snapshot is created automatically before any tweak is applied.</div>`);
    p.appendChild(e);
  } else {
    const blk = el('div', 'block');
    S.snapshots.forEach((s) => blk.appendChild(snapRow(s)));
    p.appendChild(blk);
  }
  root.appendChild(p);
}

/* ------------------------------- router ------------------------------- */
function go(view, opts = {}) {
  S.view = view;
  if (!opts.keepQuery && view.kind !== 'search') S.query = '';
  if (view.kind === 'cat') {
    const cat = catOf(view.id);
    if (!cat.subcategories.includes(S.subFilter) && S.subFilter !== 'All') S.subFilter = 'All';
  }
  renderNav();
  rerender();
  if (view.kind === 'search') setTimeout(() => $('.search-box input')?.focus(), 0);
}

function rerender() {
  const root = $('#content');
  root.innerHTML = '';
  const v = S.view;
  if (v.kind === 'home') renderHome(root);
  else if (v.kind === 'safety') renderSafety(root);
  else if (v.kind === 'backups') renderBackups(root);
  else if (v.kind === 'cat') renderCategory(root, v.id);
  else if (v.kind === 'preset') renderPreset(root, v.id);
  else if (v.kind === 'search') renderSearch(root);
  $('#content').scrollTop = 0;
}

/* ------------------------------- window chrome ------------------------------- */
$('#tb-min').onclick = () => K.windowControl('minimize');
$('#tb-max').onclick = () => K.windowControl('maximize');
$('#tb-close').onclick = () => K.windowControl('close');

document.addEventListener('keydown', (e) => {
  if ((e.ctrlKey && e.key.toLowerCase() === 'f') || (e.key === '/' && !/INPUT|TEXTAREA/.test(document.activeElement?.tagName || ''))) {
    e.preventDefault();
    if (S.view.kind !== 'search') S.view = { kind: 'search' };
    renderNav(); rerender();
    setTimeout(() => $('.search-box input')?.focus(), 0);
  }
});

/* ------------------------------- boot ------------------------------- */
(async function boot() {
  try {
    S.catalog = await K.getCatalog();
    const boot = await K.rpc('bootstrap');
    S.hw = boot.hardware;
    S.scan = boot.scan;
    S.snapshots = boot.snapshots || [];
  } catch (e) {
    $('#content').innerHTML = `<div class="empty" style="height:100%;display:grid;place-items:center"><div><div class="e-title">Engine unavailable</div><div class="e-sub">${esc(String(e.message || e))}</div><div class="e-sub" style="margin-top:8px">Build the engine first: <span style="font-family:var(--font-mono)">npm run build</span></div></div></div>`;
    return;
  }
  renderNav();
  rerender();
})();
