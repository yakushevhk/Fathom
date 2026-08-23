// ===== DEMO CLIENT ==============================================
const API_BASE = 'https://router.y7.hk/v1';
const API_KEY = 'sk-haus';
const MODEL = 'kimi/k3';

const BOTS = [
  { id: 'sdr', icon: '✉️', color: '#f59e0b', name: 'Sales Outbound', role: 'SDR', desc: 'Lead discovery, SMTP verification, CRM staging', preview: 'idle — pick a task', mockup: '/images/mockups/01_sales_outbound_sdr.png',
    prompt: 'Enrich 50 verified CISO and VP IT contacts at mid-market financial firms in London. For each: name, title, company, verified email (SMTP 250 OK). Push to CRM. Zero bounces.' },
  { id: 'intel', icon: '📡', color: '#3b82f6', name: 'Market Intelligence', role: 'Researcher', desc: '24/7 competitor pricing, features, exec hires', preview: 'idle — pick a task', mockup: '/images/mockups/02_market_intelligence.png',
    prompt: 'Monitor 15 competitors in fintech/payments for pricing changes, new features, executive hires, regulatory filings. Diff against stored state, alert on any shift.' },
  { id: 'talent', icon: '🎯', color: '#ec4899', name: 'Talent Scout', role: 'Researcher', desc: 'GitHub AST mining, dossiers with icebreakers', preview: 'idle — pick a task', mockup: '/images/mockups/03_talent_scout.png',
    prompt: 'Source 30 senior Rust/systems engineers for a robotics startup. Mine GitHub (Tokio, Axum, Polars), cross-check LinkedIn, verify emails. Deliver dossiers with commit-level icebreakers.' },
  { id: 'support', icon: '🛟', color: '#10b981', name: 'Onboarding Agent', role: 'Analyst', desc: 'Webhook config, sandbox REPL, ticket triage', preview: 'idle — pick a task', mockup: '/images/mockups/13_customer_success_onboarding.png',
    prompt: 'New client DataStream needs webhook configuration: API key provisioning, test payload verification, sandbox validation. Diagnose setup errors in an isolated REPL.' },
  { id: 'backoffice', icon: '📊', color: '#8b5cf6', name: 'Finance Ops', role: 'Analyst', desc: 'Invoice parsing, 3-way match, QuickBooks sync', preview: 'idle — pick a task', mockup: '/images/mockups/04_backoffice_invoice.png',
    prompt: 'Ingest 500 PDF vendor invoices. Cross-reference against warehouse receipts and PO records. Stage approved payments in QuickBooks. Flag discrepancies. 100% accuracy.' },
  { id: 'devops', icon: '🔧', color: '#ef4444', name: 'Software Maintainer', role: 'Developer', desc: 'AST mapping, sandbox repro, fix + test + PR', preview: 'idle — pick a task', mockup: '/images/mockups/05_devops_engineer.png',
    prompt: 'Triage Sentry zero-division error in a Python analytics CLI. Map the repo (240+ files), reproduce in sandbox, write fix + test, submit PR. All tests must pass.' },
  { id: 'legal', icon: '⚖️', color: '#06b6d4', name: 'Compliance Auditor', role: 'Verifier', desc: 'MSA audit, GDPR, liability risk matrix', preview: 'idle — pick a task', mockup: '/images/mockups/12_legal_compliance_auditor.png',
    prompt: 'Audit 200 vendor MSAs for GDPR compliance, data liability caps, non-compete clauses, jurisdiction risks. Parallel ingestion across 5 analysts. Deliver Green/Yellow/Red risk matrix.' },
  { id: 'coordinator', icon: '◈', color: '#a78bfa', name: 'Orchestrator', role: 'Coordinator', desc: 'Swarm decomposer, parallel dispatch, synthesis', preview: 'idle — pick a task', mockup: '/images/mockups/11_swarm_coordinator.png',
    prompt: 'Coordinate the full swarm: decompose an enterprise research task, delegate to specialists, verify quality, synthesize, and deliver. Use the full Fathom runtime — planning, spawning, tools, memory, governance, verification.' },
];

let activeBot = BOTS.find(b => b.id === 'coordinator');
let runCount = 0;
let totalTokens = 0;
let startTime = 0;
let isRunning = false;
let abortController = null;

// === DOM refs ===
const $ = (sel) => document.querySelector(sel);
const $$ = (sel) => document.querySelectorAll(sel);
const $x = (el, sel) => el.querySelector(sel);

const thread = $('[data-thread]');
const composer = $('[data-composer]');
const sendBtn = $('[data-send]');
const botList = $('[data-bot-list]');
const botRows = botList ? botList.querySelectorAll('[data-bot-id]') : [];
const searchInput = $('[data-bot-search]');
const activeName = $('[data-active-name]');
const activeRole = $('[data-active-role]');
const botAvatar = $('[data-bot-avatar]');
const runState = $('[data-run-state]');
const composerHint = $('[data-composer-bot]');
const computerTitle = $('[data-computer-title]');
const computerUrl = $('[data-computer-url]');
const computerBody = $('[data-computer-body]');
const computerStatus = $('[data-computer-status]');
const panelToggle = $('[data-panel-toggle]');
const panelClose = $('[data-panel-close]');
const computerPanel = $('[data-computer-panel]');
const sidebar = $('#demo-sidebar');
const menuBtn = $('.product-demo__menu-btn');
const sidebarClose = $('.product-demo__sidebar-close');
const suggestions = $$('.td-suggestion');
const dsStatus = $('[data-ds-status]');
const dsLatency = $('[data-ds-latency]');
const dsTokens = $('[data-ds-tokens]');
const dsRuns = $('[data-ds-runs]');
const pulse = $('.demo-status__pulse');
const lightbox = $('[data-lightbox]');
const lightboxImg = $('[data-lightbox-img]');
const lightboxUrl = $('[data-lightbox-url]');
const lightboxCaption = $('[data-lightbox-caption]');
const lightboxClose = $('[data-lightbox-close]');
const tabBtns = $$('[data-arch-tab]');
const tabPanes = $$('[data-arch-pane]');
const screenTake = $('[data-take-control]');
const screenTakeSm = $('[data-take-control-sm]');
const newRoutine = $('[data-new-routine]');

// === Bot selection ===
function selectBot(bot) {
  activeBot = bot;
  activeName.textContent = bot.name;
  activeRole.textContent = bot.role;
  if (botAvatar) botAvatar.textContent = bot.icon;
  if (botAvatar) botAvatar.style.background = bot.color;
  composerHint.textContent = bot.name;
  computerTitle.textContent = bot.name + "'s computer";
  computerUrl.textContent = 'fathom://' + bot.id;
  computerStatus.textContent = 'screen idle';

  // Clear computer body to mockup
  const mockup = bot.mockup;
  computerBody.innerHTML = mockup
    ? '<img src="' + mockup + '" class="pd-window-shot" alt="' + bot.name + ' screen" />'
    : '<div class="pd-window-empty"><span class="pd-we-icon">&#x25C8;</span><span>Worker idle — open a worker</span></div>';

  // Update bot time
  $$('[data-bot-time]').forEach(el => {
    if (el.dataset.botTime === bot.id) el.textContent = 'active';
    else if (el.textContent === 'active') el.textContent = 'idle';
  });

  // Active class
  botRows.forEach(row => {
    row.classList.toggle('is-active', row.dataset.botId === bot.id);
  });

  // Close sidebar on mobile
  if (window.innerWidth <= 900) sidebar.classList.remove('is-open');
}

botRows.forEach(row => {
  row.addEventListener('click', () => {
    const bot = BOTS.find(b => b.id === row.dataset.botId);
    if (bot) selectBot(bot);
  });
});

suggestions.forEach(btn => {
  btn.addEventListener('click', () => {
    const bot = BOTS.find(b => b.id === btn.dataset.botId);
    if (bot) selectBot(bot);
  });
});

// === Search ===
searchInput.addEventListener('input', () => {
  const q = searchInput.value.toLowerCase();
  botRows.forEach(row => {
    const name = $x(row, '.product-demo__bot-name');
    const desc = $x(row, '.product-demo__bot-preview');
    row.classList.toggle('is-hidden', name && desc && !name.textContent.toLowerCase().includes(q) && !desc.textContent.toLowerCase().includes(q));
  });
});

// === Messaging ===
function addMessage(type, tag, text, bot) {
  const isUser = type === 'user';
  const b = bot || activeBot;
  const time = new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  const div = document.createElement('div');
  div.className = 'pd-message pd-message--' + (isUser ? 'user' : 'bot');

  if (isUser) {
    div.innerHTML = '\
      <div class="pd-message-avatar pd-message-avatar--user">FH</div>\
      <div class="pd-bubble pd-bubble--user">' + escapeHtml(text) + '</div>\
    ';
  } else {
    const tagHtml = tag ? '<span class="pd-bubble__tag">' + tag + ' <span class="pd-bubble__time">' + time + '</span></span>' : '';
    div.innerHTML = '\
      <div class="pd-message-avatar" style="background:' + b.color + '">' + b.icon + '</div>\
      <div class="pd-bubble pd-bubble--' + (tag || 'bot') + '">\
        ' + tagHtml + '\
        <div class="pd-bubble__body">' + escapeHtml(text) + '</div>\
      </div>\
    ';
  }
  thread.appendChild(div);
  thread.scrollTop = thread.scrollHeight;
  return div;
}

function addThinking(bot) {
  const b = bot || activeBot;
  const time = new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  const div = document.createElement('div');
  div.className = 'pd-message pd-message--bot';
  div.innerHTML = '\
    <div class="pd-message-avatar" style="background:' + b.color + '">' + b.icon + '</div>\
    <div class="pd-bubble pd-bubble--thinking">\
      <span class="pd-bubble__tag">THINKING <span class="pd-bubble__time">' + time + '</span></span>\
      <div class="pd-bubble__body pd-stream-cursor"></div>\
    </div>\
  ';
  thread.appendChild(div);
  thread.scrollTop = thread.scrollHeight;
  return div;
}

function updateThinking(div, text) {
  const body = div.querySelector('.pd-bubble__body');
  if (body) body.textContent = text;
  thread.scrollTop = thread.scrollHeight;
}

function finalizeThinking(div, tag, text) {
  const bubble = div.querySelector('.pd-bubble');
  const body = div.querySelector('.pd-bubble__body');
  const time = new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  bubble.className = 'pd-bubble pd-bubble--' + (tag || 'bot');
  bubble.innerHTML = '\
    <span class="pd-bubble__tag">' + (tag || 'BOT').toUpperCase() + ' <span class="pd-bubble__time">' + time + '</span></span>\
    <div class="pd-bubble__body">' + escapeHtml(text) + '</div>\
  ';
  thread.scrollTop = thread.scrollHeight;
}

function addPlanSteps(div, steps) {
  const body = div.querySelector('.pd-bubble__body');
  body.innerHTML = steps.map(function(s, i) { return (i + 1) + '. ' + escapeHtml(s); }).join('\n');
}

function addTimestamp() {
  const time = new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  const div = document.createElement('div');
  div.className = 'pd-time';
  div.textContent = time;
  thread.appendChild(div);
}

function escapeHtml(s) {
  const div = document.createElement('div');
  div.textContent = s;
  return div.innerHTML;
}

function removeEmptyState() {
  const empty = $('.product-demo__thread-empty');
  if (empty) empty.remove();
}

// === API call ===
async function callAPI(messages, onChunk) {
  const resp = await fetch(API_BASE + '/chat/completions', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', 'Authorization': 'Bearer ' + API_KEY },
    body: JSON.stringify({
      model: MODEL,
      messages: messages,
      stream: true,
      max_tokens: 4096,
      temperature: 0.7,
    }),
    signal: abortController ? abortController.signal : null,
  });
  if (!resp.ok) {
    const err = await resp.text().catch(function() { return ''; });
    throw new Error('API error ' + resp.status + (err ? ': ' + err.slice(0, 200) : ''));
  }
  const reader = resp.body.getReader();
  const decoder = new TextDecoder();
  let buffer = '';
  let fullText = '';

  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    buffer += decoder.decode(value, { stream: true });
    const lines = buffer.split('\n');
    buffer = lines.pop() || '';
    for (const line of lines) {
      const trimmed = line.trim();
      if (!trimmed || !trimmed.startsWith('data: ')) continue;
      const data = trimmed.slice(6);
      if (data === '[DONE]') return fullText;
      try {
        const parsed = JSON.parse(data);
        const choices = parsed.choices || [];
        const delta = choices[0]?.delta?.content || '';
        if (delta) {
          fullText += delta;
          if (onChunk) onChunk(fullText);
        }
        // Track usage
        if (parsed.usage) {
          totalTokens += (parsed.usage.total_tokens || 0);
        }
      } catch (e) {}
    }
  }
  return fullText;
}

// === System prompt for each bot ===
function buildSystemPrompt(bot, userMessage) {
  const mockupContext = bot.prompt || 'Execute the task autonomously';
  return 'You are a Fathom autonomous AI worker. You are the "' + bot.name + '" worker (role: ' + bot.role + ').\n\n'
    + 'Your context: ' + bot.desc + '\n'
    + 'Your task from the user: ' + userMessage + '\n\n'
    + 'Your default objective: ' + mockupContext + '\n\n'
    + 'You are a real autonomous AI agent running in the Fathom runtime. You MUST simulate the full agent life cycle:\n\n'
    + '1. **PLAN** — Analyze the task, decompose into steps, identify what tools you need\n'
    + '2. **EXECUTE** — Use tools to gather data, process information, produce results\n'
    + '3. **ABSORB** — Store findings in memory\n'
    + '4. **VERIFY** — Validate results against quality criteria, governance rules\n'
    + '5. **DELIVER** — Present the final output to the user\n\n'
    + 'IMPORTANT: You MUST respond in a structured format. First output a PLAN section with numbered steps, then simulate tool calls with TOOL sections, then MEMORY sections for absorbed facts, then VERIFY, then FINAL.\n\n'
    + 'Use this format:\n'
    + '[PLAN]\n'
    + '1. Step one\n'
    + '2. Step two\n...\n\n'
    + '[TOOL: search_web]\n'
    + 'Query: "example search"\n'
    + 'Result: ...\n\n'
    + '[MEMORY]\n'
    + 'Absorbed: fact about company/contact\n\n'
    + '[VERIFY]\n'
    + 'Checked: quality criteria\n'
    + 'Status: PASS\n\n'
    + '[FINAL]\n'
    + 'Final deliverable here...\n\n'
    + 'Be thorough. Use multiple tools. Show real-looking data. The user is watching a live demo - make it impressive.';
}

// === Send message ===
async function sendMessage() {
  const text = composer.value.trim();
  if (!text || isRunning) return;
  composer.value = '';
  isRunning = true;
  sendBtn.classList.add('busy');
  runState.textContent = 'running';
  runState.className = 'product-demo__run-state running';
  dsStatus.textContent = 'busy';
  dsStatus.className = 'ds-v ds-v--busy';
  pulse.classList.add('active');
  startTime = Date.now();
  abortController = new AbortController();

  // Spawn sub-agents for swarm display
  const currentRunAgents = Math.floor(Math.random() * 3) + 1;
  const spawnedStart = $('[data-fleet-spawned]');
  const runningStart = $('[data-fleet-running]');
  if (spawnedStart) spawnedStart.textContent = String(parseInt(spawnedStart.textContent || '0') + currentRunAgents);
  if (runningStart) runningStart.textContent = String(parseInt(runningStart.textContent || '0') + currentRunAgents);

  removeEmptyState();
  addTimestamp();
  addMessage('user', null, text);
  const thinkingDiv = addThinking(activeBot);

  // Build system prompt
  const systemPrompt = buildSystemPrompt(activeBot, text);

  const streamHandler = function(full) {
    updateThinking(thinkingDiv, full);
  };

  try {
    const fullResponse = await callAPI([
      { role: 'system', content: systemPrompt },
      { role: 'user', content: text },
    ], streamHandler);

    if (!fullResponse) throw new Error('Empty response');

    // Parse structured response into tagged bubbles
    const sections = fullResponse.split(/(?=\[PLAN\]|\[TOOL|\[MEMORY\]|\[VERIFY\]|\[FINAL\])/);
    let thinkingFinalized = false;
    let hasFinal = false;
    let memEl = null;

    for (const section of sections) {
      const trimmed = section.trim();
      if (!trimmed) continue;

      if (trimmed.startsWith('[PLAN]')) {
        finalizeThinking(thinkingDiv, 'plan', trimmed.replace('[PLAN]', '').trim());
        thinkingFinalized = true;
      } else if (trimmed.startsWith('[TOOL')) {
        if (!thinkingFinalized) { finalizeThinking(thinkingDiv, 'info', trimmed); thinkingFinalized = true; }
        const tagMatch = trimmed.match(/\[TOOL:\s*([^\]]+)\]/);
        const tag = tagMatch ? 'tool: ' + tagMatch[1] : 'tool';
        addMessage('bot', tag, trimmed.replace(/\[TOOL:[^\]]*\]/, '').trim(), activeBot);
      } else if (trimmed.startsWith('[MEMORY]')) {
        if (!thinkingFinalized) { finalizeThinking(thinkingDiv, 'info', trimmed); thinkingFinalized = true; }
        addMessage('bot', 'memory', trimmed.replace('[MEMORY]', '').trim(), activeBot);
        // Update memory count
        memEl = $('[data-mem-facts]');
        if (memEl) memEl.textContent = String(parseInt(memEl.textContent || '0') + 1);
      } else if (trimmed.startsWith('[VERIFY]')) {
        if (!thinkingFinalized) { finalizeThinking(thinkingDiv, 'info', trimmed); thinkingFinalized = true; }
        addMessage('bot', 'verify', trimmed.replace('[VERIFY]', '').trim(), activeBot);
      } else if (trimmed.startsWith('[FINAL]')) {
        if (!thinkingFinalized) { finalizeThinking(thinkingDiv, 'done', trimmed.replace('[FINAL]', '').trim()); thinkingFinalized = true; }
        addMessage('bot', 'done', trimmed.replace('[FINAL]', '').trim(), activeBot);
        hasFinal = true;
      } else {
        // Remaining generic text
        if (hasFinal) {
          addMessage('bot', 'info', trimmed, activeBot);
        }
      }
    }

    // If thinking div never finalized (no PLAN/FINAL section), use the full response
    if (!thinkingFinalized) {
      finalizeThinking(thinkingDiv, hasFinal ? 'info' : 'done', fullResponse);
    }

    // Update computer screen with mockup
    const mockup = activeBot.mockup;
    if (mockup) {
      computerBody.innerHTML = '<img src="' + mockup + '" class="pd-window-shot" alt="' + activeBot.name + ' screen" />';
    } else {
      computerBody.innerHTML = '';
    }
    computerStatus.textContent = 'task complete';

    // Update stats
    runCount++;
    runState.textContent = 'done';
    runState.className = 'product-demo__run-state done';
    dsStatus.textContent = 'idle';
    dsStatus.className = 'ds-v ds-v--ok';
    pulse.classList.remove('active');
    dsRuns.textContent = runCount;
    dsTokens.textContent = totalTokens;
    const elapsedRaw = (Date.now() - startTime) / 1000;
    const mins = Math.floor(elapsedRaw / 60);
    const secs = Math.floor(elapsedRaw % 60);
    dsLatency.textContent = String(mins).padStart(2, '0') + ':' + String(secs).padStart(2, '0');

    // Update swarm metrics
    const completed = $('[data-fleet-completed]');
    const tools = $('[data-fleet-tools]');
    const running = $('[data-fleet-running]');
    const spawned = $('[data-fleet-spawned]');
    if (completed) completed.textContent = String(parseInt(completed.textContent || '0') + currentRunAgents);
    if (running) running.textContent = String(Math.max(0, parseInt(running.textContent || '0') - currentRunAgents));
    const toolCount = (fullResponse.match(/\[TOOL:/g) || []).length;
    if (tools) tools.textContent = String(parseInt(tools.textContent || '0') + toolCount);

    // Update stats cards
    const statRuns = $('[data-stat-runs]');
    const statTools = $('[data-stat-tools]');
    const statTokens = $('[data-stat-tokens]');
    const statLatency = $('[data-stat-latency]');
    const statAgents = $('[data-stat-agents]');
    const statMemory = $('[data-stat-memory]');
    if (statRuns) statRuns.textContent = runCount;
    if (statTools) statTools.textContent = tools ? tools.textContent : '0';
    if (statTokens) statTokens.textContent = totalTokens;
    if (statLatency) statLatency.textContent = String(mins).padStart(2, '0') + ':' + String(secs).padStart(2, '0');
    if (statAgents) statAgents.textContent = spawned ? spawned.textContent : '0';
    if (statMemory) statMemory.textContent = memEl ? memEl.textContent : '0';

    // Audit
    const allowed = $('[data-audit-allowed]');
    if (allowed) allowed.textContent = String(parseInt(allowed.textContent || '0') + toolCount);
    const denied = $('[data-audit-denied]');
    if (denied && denied.textContent === '0') denied.textContent = '0';
    const memGc = $('[data-mem-gc]');
    if (memGc && memGc.textContent === '—') memGc.textContent = '142s to next GC';

    // Memory stats
    const memConf = $('[data-mem-confidence]');
    if (memConf && memConf.textContent === '—') memConf.textContent = '0.92';

  } catch (err) {
    pulse.classList.remove('active');
    const failed = $('[data-fleet-failed]');
    if (failed) failed.textContent = String(parseInt(failed.textContent || '0') + currentRunAgents);
    if (err.name === 'AbortError') {
      finalizeThinking(thinkingDiv, 'info', 'Cancelled');
    } else {
      finalizeThinking(thinkingDiv, 'error', 'Error: ' + err.message);
      runState.textContent = 'error';
      runState.className = 'product-demo__run-state error';
      dsStatus.textContent = 'error';
      dsStatus.className = 'ds-v ds-v--err';
    }
  } finally {
    isRunning = false;
    sendBtn.classList.remove('busy');
    abortController = null;
  }
}

// === Event listeners ===
sendBtn.addEventListener('click', sendMessage);
composer.addEventListener('keydown', function(e) {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault();
    sendMessage();
  }
});

// === Panel toggle ===
panelToggle.addEventListener('click', function() {
  const pressed = panelToggle.getAttribute('aria-pressed') === 'true';
  panelToggle.setAttribute('aria-pressed', String(!pressed));
  if (pressed) {
    computerPanel.style.display = 'none';
    panelToggle.style.color = 'var(--text-tertiary)';
  } else {
    computerPanel.style.display = 'flex';
    computerPanel.classList.remove('is-open');
    panelToggle.style.color = 'var(--accent)';
  }
});

panelClose.addEventListener('click', function() {
  computerPanel.style.display = 'none';
  panelToggle.setAttribute('aria-pressed', 'false');
  panelToggle.style.color = 'var(--text-tertiary)';
});

// === Sidebar toggle mobile ===
if (menuBtn) {
  menuBtn.addEventListener('click', function() {
    sidebar.classList.toggle('is-open');
  });
}
if (sidebarClose) {
  sidebarClose.addEventListener('click', function() {
    sidebar.classList.remove('is-open');
  });
}

// === Screen lightbox ===
screenTake.addEventListener('click', openLightbox);
screenTakeSm.addEventListener('click', openLightbox);

function openLightbox() {
  const mockup = activeBot.mockup;
  if (mockup) {
    lightboxImg.src = mockup;
    lightboxUrl.textContent = 'fathom://' + activeBot.id + '/screen';
    lightboxCaption.textContent = activeBot.name + ' — take control';
    lightbox.removeAttribute('hidden');
  }
}

lightboxClose.addEventListener('click', function() {
  lightbox.setAttribute('hidden', '');
});

lightbox.addEventListener('click', function(e) {
  if (e.target === lightbox) lightbox.setAttribute('hidden', '');
});

// === Architecture tabs ===
tabBtns.forEach(function(btn) {
  btn.addEventListener('click', function() {
    const tab = btn.dataset.archTab;
    tabBtns.forEach(function(b) { b.classList.remove('tab--active'); });
    btn.classList.add('tab--active');
    tabPanes.forEach(function(p) { p.classList.remove('tab-pane--active'); });
    const pane = $('[data-arch-pane="' + tab + '"]');
    if (pane) pane.classList.add('tab-pane--active');
  });
});

// === New routine ===
if (newRoutine) {
  newRoutine.addEventListener('click', function() {
    const name = prompt('Routine name:');
    if (!name) return;
    const when = prompt('Schedule (e.g. "Daily 8am"):');
    if (!when) return;
    const div = document.createElement('button');
    div.type = 'button';
    div.className = 'product-demo__routine';
    div.innerHTML = '<span class="product-demo__routine-icon">&#x25F7;</span><span class="product-demo__routine-name">' + escapeHtml(name) + '</span><span class="product-demo__routine-when">' + escapeHtml(when) + '</span>';
    newRoutine.parentNode.insertBefore(div, newRoutine);
  });
}

// === Handle Escape to close sidebar on mobile ===
document.addEventListener('keydown', function(e) {
  if (e.key === 'Escape') {
    if (sidebar) sidebar.classList.remove('is-open');
    if (computerPanel) {
      computerPanel.style.display = '';
      computerPanel.classList.remove('is-open');
    }
    lightbox.setAttribute('hidden', '');
  }
});

// === Initial active state ===
selectBot(activeBot);