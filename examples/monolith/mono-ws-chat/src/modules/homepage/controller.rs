use ferrite_framework::{controller, impl_controller, inject};

use crate::app_service::AppService;

#[controller("")]
pub struct HomePageController {
    #[allow(dead_code)]
    app: AppService,
}

#[impl_controller]
impl HomePageController {
    #[inject]
    pub fn new(app: AppService) -> Self {
        Self { app }
    }

    #[get("/")]
    pub async fn index(&self) -> axum::response::Html<String> {
        axum::response::Html(chat_html())
    }
}

fn chat_html() -> String {
    r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8" />
<meta name="viewport" content="width=device-width, initial-scale=1.0" />
<title>Ferrite WS Chat</title>
<link rel="stylesheet" type="text/css" href="https://cdn.jsdelivr.net/npm/toastify-js/src/toastify.min.css">
<script src="https://cdn.tailwindcss.com"></script>
<script src="https://unpkg.com/htmx.org@1.9.12/dist/htmx.min.js"></script>
<script src="https://unpkg.com/htmx.org@1.9.12/dist/ext/ws.js"></script>
<script type="text/javascript" src="https://cdn.jsdelivr.net/npm/toastify-js"></script>
<script>
  tailwind.config = {
    theme: {
      extend: {
        colors: {
          cream: { 50:'#fdf8f3', 100:'#f9efe3', 200:'#f1dfc7', 300:'#e5c99f', 400:'#d6ac72', 500:'#c89253', 600:'#a77341', 700:'#7f5730', 800:'#5a3e22', 900:'#3c2917' },
          coffee:{ 900:'#2a1a10', 800:'#3f2a1b', 700:'#583b26', 600:'#75503a' }
        },
        boxShadow: { 'soft':'0 10px 40px -20px rgba(60,41,23,.35)', 'card':'0 20px 60px -25px rgba(60,41,23,.45)' }
      },
      keyframes: {
        fadeIn:  { '0%':{opacity:0, transform:'translateY(8px)'}, '100%':{opacity:1, transform:'translateY(0)'} },
        popIn:   { '0%':{opacity:0, transform:'scale(.96)'},    '100%':{opacity:1, transform:'scale(1)'} }
      },
      animation: { 'fade-in':'fadeIn .35s ease-out both', 'pop-in':'popIn .4s cubic-bezier(.2,.9,.3,1.2) both' }
    }
  }
</script>
<style>
  body { font-family: ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto, "Helvetica Neue", Arial; position: relative; }
  .scroll-slim::-webkit-scrollbar { width: 8px; height: 8px; }
  .scroll-slim::-webkit-scrollbar-thumb { background: rgba(120, 80, 40, .35); border-radius: 9999px; }
  .toastify { border-radius: 14px !important; padding: 10px 18px !important; font-family: inherit !important; box-shadow: 0 12px 40px -15px rgba(0,0,0,.35) !important; font-size: 14px !important; }
  .auth-tab-active { background-color: #3f2a1b !important; color: #fdf8f3 !important; }
  .auth-section { transition: opacity .3s ease, transform .3s ease, visibility .3s ease; min-height: 100vh; width: 100%; }
  .auth-hidden { opacity: 0; visibility: hidden; transform: scale(.98) translateY(8px); position: fixed; inset: 0; pointer-events: none; overflow: hidden; max-height: 0; }
</style>
</head>
<body class="bg-cream-50 min-h-screen text-coffee-900">

<!-- ===================== AUTH VIEW (FULL SCREEN) ===================== -->
<div id="auth-view" class="auth-section min-h-screen flex items-center justify-center p-4 bg-[radial-gradient(ellipse_at_top,rgba(200,146,83,0.12),transparent_60%)]">
  <div class="w-full max-w-md animate-pop-in">
    <div class="text-center mb-7">
      <div class="inline-flex items-center justify-center w-16 h-16 rounded-2xl bg-coffee-800 text-cream-50 shadow-card mb-4">
        <svg xmlns="http://www.w3.org/2000/svg" class="w-8 h-8" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z"/></svg>
      </div>
      <h1 class="text-2xl font-bold text-coffee-900 tracking-tight">Ferrite WS Chat</h1>
      <p class="text-sm text-coffee-600/80 mt-1">ferrite-ws · htmx · tailwind cdn</p>
    </div>

    <div class="bg-white rounded-3xl shadow-card border border-cream-200 p-7">
      <!-- Tab buttons -->
      <div class="grid grid-cols-2 gap-2 p-1 bg-cream-100 rounded-2xl mb-6">
        <button id="tab-login"    class="py-2.5 rounded-xl text-sm font-semibold transition auth-tab-active">Log in</button>
        <button id="tab-register" class="py-2.5 rounded-xl text-sm font-semibold text-coffee-700 transition hover:bg-cream-200/60">Sign up</button>
      </div>

      <!-- FORM -->
      <form id="auth-form" class="space-y-4" onsubmit="return false;">
        <!-- Name field (only register) -->
        <div id="field-name" class="space-y-2">
          <label class="block text-xs font-semibold text-coffee-700">Full name</label>
          <input id="name" type="text" placeholder="Jane Doe" autocomplete="name"
            class="w-full rounded-xl border border-cream-200 bg-white px-4 py-3 text-sm focus:outline-none focus:ring-2 focus:ring-coffee-600/40 focus:border-coffee-400 transition" />
        </div>

        <div class="space-y-2">
          <label class="block text-xs font-semibold text-coffee-700">Email</label>
          <input id="email" type="email" placeholder="you@example.com" autocomplete="username"
            class="w-full rounded-xl border border-cream-200 bg-white px-4 py-3 text-sm focus:outline-none focus:ring-2 focus:ring-coffee-600/40 focus:border-coffee-400 transition" />
        </div>

        <div class="space-y-2">
          <label class="block text-xs font-semibold text-coffee-700">Password <span class="text-coffee-500/70 font-normal">(min. 6 characters)</span></label>
          <input id="password" type="password" placeholder="••••••••" autocomplete="current-password" minlength="6"
            class="w-full rounded-xl border border-cream-200 bg-white px-4 py-3 text-sm focus:outline-none focus:ring-2 focus:ring-coffee-600/40 focus:border-coffee-400 transition" />
        </div>

        <div id="auth-error" class="text-sm text-red-700/90 bg-red-50 border border-red-200 rounded-xl p-3 min-h-[1rem] hidden"></div>

        <!-- Submit button (dynamic text) -->
        <button id="btn-submit" type="submit"
          class="w-full mt-2 py-3.5 rounded-xl bg-coffee-800 hover:bg-coffee-900 text-cream-50 text-sm font-semibold shadow-md hover:shadow-lg transition-all transform hover:-translate-y-0.5 active:translate-y-0 flex items-center justify-center gap-2">
          <svg id="spinner" class="w-4 h-4 animate-spin hidden" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24"><circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle><path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"></path></svg>
          <span id="btn-submit-text">Log in</span>
        </button>
      </form>

      <div class="relative my-6">
        <div class="absolute inset-0 flex items-center"><div class="w-full border-t border-cream-200"></div></div>
        <div class="relative flex justify-center text-xs"><span class="bg-white px-3 text-coffee-600/70">or</span></div>
      </div>

      <!-- Guest button -->
      <button id="btn-guest"
        class="w-full py-3 rounded-xl border-2 border-dashed border-cream-300 hover:border-coffee-500 hover:bg-cream-100/60 text-coffee-800 text-sm font-medium transition flex items-center justify-center gap-2">
        <svg xmlns="http://www.w3.org/2000/svg" class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z"/></svg>
        Continue as guest
      </button>

      <p class="mt-6 text-[11px] text-center text-coffee-600/60 leading-relaxed">
        Demo API:
        <span class="inline-block mx-0.5 px-1.5 py-0.5 rounded bg-cream-100 mt-0.5">POST /auth/register</span>
        <span class="inline-block mx-0.5 px-1.5 py-0.5 rounded bg-cream-100 mt-0.5">POST /auth/login</span>
        <span class="inline-block mx-0.5 px-1.5 py-0.5 rounded bg-cream-100 mt-0.5">ws://…/ws/chat</span>
        <span class="inline-block mx-0.5 px-1.5 py-0.5 rounded bg-cream-100 mt-0.5">GET /messages</span>
      </p>
    </div>

    <p class="text-center text-[11px] text-coffee-500/60 mt-5">© 2025 Ferrite Framework · WebSocket Gateway Demo</p>
  </div>
</div>

<!-- ===================== CHAT VIEW (SHOWN POST-AUTH) ===================== -->
<div id="chat-view" class="auth-section auth-hidden min-h-screen p-4">
  <div class="max-w-6xl mx-auto h-[94vh] flex flex-col animate-fade-in">
    <!-- Chat top bar -->
    <header class="bg-white rounded-2xl rounded-b-none shadow-soft border border-cream-200 border-b-0 px-6 py-4 flex items-center justify-between gap-3">
      <div class="flex items-center gap-3">
        <div class="w-10 h-10 rounded-xl bg-coffee-800 text-cream-50 grid place-items-center font-bold">F</div>
        <div>
          <div class="font-semibold text-coffee-900 leading-tight">Lounge</div>
          <div id="chat-user-badge" class="text-xs text-coffee-600/80">—</div>
        </div>
      </div>
      <div class="flex items-center gap-3">
        <div class="hidden sm:flex items-center gap-2 text-xs bg-cream-100/60 rounded-xl px-3 py-1.5">
          <span id="ws-dot" class="w-2 h-2 rounded-full bg-amber-400 inline-block"></span>
          <span id="ws-status" class="text-coffee-700">connecting…</span>
        </div>
        <button id="btn-logout"
          class="inline-flex items-center gap-1.5 px-3.5 py-2 rounded-xl text-sm font-medium bg-cream-100 hover:bg-red-50 hover:text-red-700 border border-cream-200 hover:border-red-200 transition">
          <svg xmlns="http://www.w3.org/2000/svg" class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M17 16l4-4m0 0l-4-4m4 4H7m6 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1"/></svg>
          Sign out
        </button>
      </div>
    </header>

    <!-- Messages + input -->
    <section class="flex-1 flex flex-col bg-white rounded-2xl rounded-t-none shadow-card border border-cream-200 overflow-hidden">
      <div id="messages" class="flex-1 overflow-y-auto px-6 py-5 space-y-3 bg-cream-50/60 scroll-slim">
        <div class="text-center text-xs text-coffee-600/70 py-10">
          <div class="inline-flex flex-col items-center gap-2">
            <div class="w-12 h-12 rounded-2xl bg-cream-100 grid place-items-center">
              <svg xmlns="http://www.w3.org/2000/svg" class="w-6 h-6 text-coffee-500" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z"/></svg>
            </div>
            No messages yet. Say hi and start the conversation!
          </div>
        </div>
      </div>

      <form id="chat-form" class="p-4 sm:p-5 border-t border-cream-100 flex items-end gap-2" hx-ext="ws" ws-connect="">
        <textarea id="chat-input" rows="2" name="text" required placeholder="Type a message… (Enter to send, Shift+Enter newline)"
          class="flex-1 resize-none rounded-2xl border border-cream-200 bg-white px-4 py-3 text-sm focus:outline-none focus:ring-2 focus:ring-coffee-600/40 focus:border-coffee-400 transition"></textarea>
        <button id="btn-send"
          class="h-[52px] px-5 rounded-2xl bg-coffee-800 hover:bg-coffee-900 text-cream-50 text-sm font-semibold shadow-md hover:shadow-lg transition-all transform hover:-translate-y-0.5 active:translate-y-0 inline-flex items-center gap-1.5"
          type="submit">
          <svg xmlns="http://www.w3.org/2000/svg" class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M12 19l9 2-9-18-9 18 9-2zm0 0v-8"/></svg>
          Send
        </button>
      </form>
    </section>
  </div>
</div>

<script>
  const TOKEN_KEY = 'ferrite.chat.token';
  const ME_KEY    = 'ferrite.chat.me';
  const $  = (id) => document.getElementById(id);

  const state = {
    token: localStorage.getItem(TOKEN_KEY) || '',
    me:    null,
    mode:  'login'  // login | register
  };
  try { state.me = JSON.parse(localStorage.getItem(ME_KEY) || 'null'); } catch {}

  // ========== Toastify helpers ==========
  function toastSuccess(text) {
    Toastify({
      text: text,
      duration: 2800,
      gravity: 'top',
      position: 'right',
      style: { background: 'linear-gradient(135deg,#3f2a1b,#5a3e22)', borderRadius: '14px' },
      stopOnFocus: true,
      avatar: 'data:image/svg+xml;utf8,' + encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>')
    }).showToast();
  }
  function toastError(text) {
    Toastify({
      text: text,
      duration: 3500,
      gravity: 'top',
      position: 'right',
      style: { background: 'linear-gradient(135deg,#7f1d1d,#991b1b)', borderRadius: '14px' },
      stopOnFocus: true,
      avatar: 'data:image/svg+xml;utf8,' + encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="15" y1="9" x2="9" y2="15"/><line x1="9" y1="9" x2="15" y2="15"/></svg>')
    }).showToast();
  }
  function toastInfo(text) {
    Toastify({
      text: text,
      duration: 2400,
      gravity: 'top',
      position: 'right',
      style: { background: 'linear-gradient(135deg,#7f5730,#a77341)', borderRadius: '14px' },
      stopOnFocus: true
    }).showToast();
  }

  // ========== Tab switching ==========
  function setMode(mode) {
    state.mode = mode;
    if (mode === 'login') {
      $('tab-login').classList.add('auth-tab-active');
      $('tab-register').classList.remove('auth-tab-active');
      $('field-name').style.display = 'none';
      $('btn-submit-text').textContent = 'Log in';
    } else {
      $('tab-register').classList.add('auth-tab-active');
      $('tab-login').classList.remove('auth-tab-active');
      $('field-name').style.display = '';
      $('btn-submit-text').textContent = 'Create account';
    }
    $('auth-error').classList.add('hidden');
    $('auth-error').textContent = '';
  }
  setMode('login');
  $('tab-login').addEventListener('click',    () => setMode('login'));
  $('tab-register').addEventListener('click', () => setMode('register'));

  // ========== View switching ==========
  function showAuthView() {
    $('chat-view').classList.add('auth-hidden');
    $('auth-view').classList.remove('auth-hidden');
  }
  function showChatView() {
    $('auth-view').classList.add('auth-hidden');
    $('chat-view').classList.remove('auth-hidden');
    // badge
    const badge = $('chat-user-badge');
    if (state.token && state.me) {
      badge.innerHTML = `<span class="inline-flex items-center gap-1"><span class="w-1.5 h-1.5 rounded-full bg-emerald-500"></span> Signed in · <b>${escapeHtml(state.me.name)}</b> · ${escapeHtml(state.me.email)}</span>`;
    } else if (state.me) {
      badge.innerHTML = `<span class="inline-flex items-center gap-1"><span class="w-1.5 h-1.5 rounded-full bg-amber-400"></span> Guest · <b>${escapeHtml(state.me.name)}</b></span>`;
    }
    setTimeout(scrollToBottom, 120);
  }

  // ========== Auth API ==========
  async function postJSON(url, body) {
    const res = await fetch(url, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    });
    const data = await res.json().catch(() => ({}));
    if (!res.ok) {
      const msg = (data && (data.message || data.error)) || `HTTP ${res.status}`;
      throw new Error(msg);
    }
    return data;
  }

  function setLoading(loading) {
    $('btn-submit').disabled = loading;
    $('btn-guest').disabled = loading;
    $('spinner').classList.toggle('hidden', !loading);
    $('btn-submit').classList.toggle('opacity-60', loading);
    $('btn-submit').classList.toggle('cursor-not-allowed', loading);
  }

  function persistAndGo(data, okMsg, delay = 1100) {
    state.token = data.token || '';
    state.me    = { id: data.user_id, email: data.email, name: data.name || data.email.split('@')[0] };
    if (state.token) localStorage.setItem(TOKEN_KEY, state.token);
    localStorage.setItem(ME_KEY, JSON.stringify(state.me));
    $('auth-error').classList.add('hidden');
    $('auth-error').textContent = '';
    toastSuccess(okMsg);
    setLoading(false);
    reconnectWs();
    setTimeout(showChatView, delay);
  }

  function guestIdentity() {
    const a = Math.random().toString(36).slice(2, 8);
    state.token = '';
    state.me    = { id: 0, email: a + '@local', name: 'guest-' + a };
    localStorage.removeItem(TOKEN_KEY);
    localStorage.setItem(ME_KEY, JSON.stringify(state.me));
    toastInfo(`Entering as guest (${state.me.name})`);
    setLoading(false);
    reconnectWs();
    setTimeout(showChatView, 650);
  }

  // Submit form
  $('auth-form').addEventListener('submit', async () => {
    const email = $('email').value.trim();
    const pwd   = $('password').value;
    const name  = $('name').value.trim();
    const errEl = $('auth-error');
    errEl.classList.add('hidden');
    errEl.textContent = '';

    if (!email || !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email)) {
      errEl.textContent = 'Please enter a valid email address.';
      errEl.classList.remove('hidden'); toastError('Invalid email'); return;
    }
    if (!pwd || pwd.length < 6) {
      errEl.textContent = 'Password must be at least 6 characters long.';
      errEl.classList.remove('hidden'); toastError('Password too short'); return;
    }
    if (state.mode === 'register' && name && name.length < 2) {
      errEl.textContent = 'Full name must be at least 2 characters long.';
      errEl.classList.remove('hidden'); toastError('Name too short'); return;
    }

    setLoading(true);
    try {
      if (state.mode === 'register') {
        const d = await postJSON('/auth/register', { email, password: pwd, name: name || email.split('@')[0] });
        persistAndGo(d, `🎉 Sign up successful! Welcome, ${d.name || email.split('@')[0]}`);
      } else {
        const d = await postJSON('/auth/login',    { email, password: pwd });
        persistAndGo(d, `✅ Log in successful! Welcome back, ${d.name || email.split('@')[0]}`, 900);
      }
    } catch (e) {
      setLoading(false);
      errEl.textContent = (state.mode === 'register' ? 'Sign up failed: ' : 'Log in failed: ') + e.message;
      errEl.classList.remove('hidden');
      toastError(e.message);
    }
  });

  $('btn-guest').addEventListener('click', () => {
    setLoading(true);
    guestIdentity();
  });

  // Logout
  $('btn-logout').addEventListener('click', () => {
    state.token = ''; state.me = null;
    localStorage.removeItem(TOKEN_KEY); localStorage.removeItem(ME_KEY);
    if (ws) { try { ws.close(); } catch {} ws = null; }
    toastInfo('You have been signed out');
    setMode('login');
    $('email').value = ''; $('password').value = ''; $('name').value = '';
    showAuthView();
  });

  // ========== WebSocket ==========
  let ws = null;
  function wsUrl() {
    const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
    let url = `${proto}//${location.host}/ws/chat`;
    if (state.token) url += '?token=' + encodeURIComponent(state.token);
    return url;
  }
  const wsDot    = $('ws-dot');
  const wsStatus = $('ws-status');
  function setWsStatus(text, color) {
    if (wsStatus) wsStatus.textContent = text;
    if (wsDot)    wsDot.className    = `w-2 h-2 rounded-full inline-block ${color}`;
  }
  function reconnectWs() {
    if (ws) { try { ws.close(); } catch {} ws = null; }
    const url = wsUrl();
    setWsStatus('connecting…', 'bg-amber-400');
    try {
      ws = new WebSocket(url);
    } catch (e) {
      setWsStatus('error: ' + e.message, 'bg-red-500');
      return;
    }
    ws.addEventListener('open',  () => {
      setWsStatus('online · ' + (state.me ? state.me.name : 'anonymous'), 'bg-emerald-500');
    });
    ws.addEventListener('close', () => {
      setWsStatus('disconnected (retrying in 2s)', 'bg-red-500');
      setTimeout(reconnectWs, 2000);
    });
    ws.addEventListener('error', () => setWsStatus('socket error', 'bg-red-500'));
    ws.addEventListener('message', (ev) => {
      let data;
      try { data = JSON.parse(ev.data); } catch { return; }
      const event   = data.event;
      const payload = data.data || {};
      if (event === 'chat:message') {
        appendMessage(payload);
      } else if (event === 'chat:history') {
        clearMessages();
        (payload.messages || []).forEach(appendMessage);
        if (!(payload.messages || []).length) appendPlaceholder();
        scrollToBottom();
      } else if (event === 'chat:system') {
        appendSystem(payload.text || 'system');
      } else if (event === 'chat:error') {
        appendSystem('⚠ ' + (payload.message || 'error'));
      }
    });
  }

  // ========== Messages rendering ==========
  const messagesEl = $('messages');
  const chatForm   = $('chat-form');
  const chatInput  = $('chat-input');

  function appendPlaceholder() {
    clearMessages();
    const wrap = document.createElement('div');
    wrap.className = 'text-center text-xs text-coffee-600/70 py-10';
    wrap.innerHTML = `
      <div class="inline-flex flex-col items-center gap-2">
        <div class="w-12 h-12 rounded-2xl bg-cream-100 grid place-items-center">
          <svg xmlns="http://www.w3.org/2000/svg" class="w-6 h-6 text-coffee-500" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z"/></svg>
        </div>
        No messages yet. Say hi and start the conversation!
      </div>`;
    messagesEl.appendChild(wrap);
  }
  function clearMessages() { messagesEl.innerHTML = ''; }
  function isOwn(m) {
    if (!state.me) return false;
    if (state.me.id && m.user_id === state.me.id) return true;
    return (m.name || '').toLowerCase() === (state.me.name || '').toLowerCase();
  }
  function appendMessage(m) {
    if (messagesEl.firstChild && messagesEl.firstChild.querySelector('.w-12.h-12.rounded-2xl')) messagesEl.innerHTML = '';
    const own  = isOwn(m);
    const name = m.name || (m.email || '').split('@')[0] || 'anon';
    const row  = document.createElement('div');
    row.className = 'flex ' + (own ? 'justify-end' : 'justify-start') + ' animate-fade-in';
    const box = document.createElement('div');
    box.className = 'max-w-[78%] rounded-2xl px-4 py-2.5 text-sm shadow-sm ' +
      (own ? 'bg-coffee-800 text-cream-50 rounded-br-sm' : 'bg-white border border-cream-200 rounded-bl-sm');
    const meta = document.createElement('div');
    meta.className = 'text-[11px] mb-1 ' + (own ? 'text-cream-100/80' : 'text-coffee-600/70');
    const ts = m.ts ? new Date(m.ts * 1000) : new Date();
    const hh = ts.getHours().toString().padStart(2,'0');
    const mm = ts.getMinutes().toString().padStart(2,'0');
    meta.textContent = `${escapeHtml(name)} · ${hh}:${mm}`;
    const body = document.createElement('div');
    body.className = 'whitespace-pre-wrap break-words leading-snug';
    body.textContent = m.text || '';
    box.appendChild(meta);
    box.appendChild(body);
    row.appendChild(box);
    messagesEl.appendChild(row);
    scrollToBottom();
  }
  function appendSystem(text) {
    const row = document.createElement('div');
    row.className = 'text-center text-xs text-coffee-600/80 animate-fade-in';
    row.textContent = text;
    messagesEl.appendChild(row);
    scrollToBottom();
  }
  function scrollToBottom() {
    requestAnimationFrame(() => { messagesEl.scrollTop = messagesEl.scrollHeight; });
  }
  function escapeHtml(s) {
    return String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;' }[c]));
  }

  // ========== Chat form send ==========
  chatForm.addEventListener('submit', (e) => {
    e.preventDefault();
    const text = chatInput.value.trim();
    if (!text) return;
    if (!ws || ws.readyState !== WebSocket.OPEN) {
      appendSystem('🔌 Not connected. Retrying…');
      reconnectWs();
      return;
    }
    const payload = { event: 'chat:send', data: { text } };
    if (state.token) payload.data.token = state.token;
    try { ws.send(JSON.stringify(payload)); } catch { appendSystem('Failed to send message.'); }
    chatInput.value = '';
    chatInput.focus();
  });
  chatInput.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      chatForm.requestSubmit();
    }
  });

  // ========== Boot: if user already authenticated go to chat directly ==========
  if (state.token && state.me) {
    // Existing session: jump straight to chat
    showChatView();
    reconnectWs();
  } else if (state.me && !state.token) {
    // Guest was saved: show quick confirm
    showAuthView();
  }
</script>
</body>
</html>"#.to_string()
}
