// ===== DEMO CLIENT (ENHANCED) ==============================================
const API_BASE = '/api';  // proxied via Vercel Edge Function
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

let activeBot = BOTS.find(b => b.id === 'coordinator') || BOTS[0];
let runCount = 0;
let totalTokens = 0;
let startTime = 0;
let isRunning = false;
let abortController = null;
let currentPendingApprovalResolver = null;

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

// === Lightweight Markdown Parser ===
function renderMarkdown(raw) {
  if (!raw) return '';
  let text = escapeHtml(raw);

  // Bold & Italic
  text = text.replace(/\*\*\*(.*?)\*\*\*/g, '<strong><em>$1</em></strong>');
  text = text.replace(/\*\*(.*?)\*\*/g, '<strong>$1</strong>');
  text = text.replace(/\*(.*?)\*/g, '<em>$1</em>');
  text = text.replace(/`([^`]+)`/g, '<code>$1</code>');

  // Simple table parsing
  const lines = text.split('\n');
  let inTable = false;
  let tableHtml = '';
  let out = [];

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i].trim();
    if (line.startsWith('|') && line.endsWith('|')) {
      const cells = line.slice(1, -1).split('|').map(c => c.trim());
      if (cells.every(c => /^:?-+:?$/.test(c))) {
        // Divider row
        continue;
      }
      if (!inTable) {
        inTable = true;
        tableHtml = '<table class="pd-table"><thead><tr>' + cells.map(c => '<th>' + c + '</th>').join('') + '</tr></thead><tbody>';
      } else {
        tableHtml += '<tr>' + cells.map(c => '<td>' + c + '</td>').join('') + '</tr>';
      }
    } else {
      if (inTable) {
        tableHtml += '</tbody></table>';
        out.push(tableHtml);
        inTable = false;
      }
      out.push(line);
    }
  }
  if (inTable) {
    tableHtml += '</tbody></table>';
    out.push(tableHtml);
  }

  return out.join('<br>');
}

// === Dynamic Arch Tab Updaters ===
function appendSwarmNode(agentName, role, status, depth, taskSnippet) {
  const treeContainer = $('[data-swarm-tree]');
  if (!treeContainer) return;
  const empty = treeContainer.querySelector('.swarm-empty');
  if (empty) empty.remove();

  let list = treeContainer.querySelector('.swarm-node-list');
  if (!list) {
    list = document.createElement('div');
    list.className = 'swarm-node-list';
    treeContainer.appendChild(list);
  }

  const node = document.createElement('div');
  node.className = 'swarm-node swarm-node--depth-' + depth;
  node.innerHTML = `
    <div class="sn-icon">◈</div>
    <div class="sn-info">
      <div class="sn-title">
        <span class="sn-name">${escapeHtml(agentName)}</span>
        <span class="sn-role">${escapeHtml(role)}</span>
        <span class="sn-status is-${status}">${escapeHtml(status)}</span>
      </div>
      <div class="sn-task">${escapeHtml(taskSnippet || 'Sub-agent task execution')}</div>
    </div>
  `;
  list.appendChild(node);
}

function appendMemoryFact(fact, scope, confidence) {
  const memContainer = $('[data-memory-list]');
  if (!memContainer) return;
  const empty = memContainer.querySelector('.memory-empty');
  if (empty) empty.remove();

  let list = memContainer.querySelector('.memory-fact-list');
  if (!list) {
    list = document.createElement('div');
    list.className = 'memory-fact-list';
    memContainer.appendChild(list);
  }

  const factEl = document.createElement('div');
  factEl.className = 'memory-fact-card';
  const confVal = confidence || '0.94';
  factEl.innerHTML = `
    <div class="mf-head">
      <span class="mf-scope">${escapeHtml(scope || 'entity')}</span>
      <span class="mf-conf">conf: ${confVal}</span>
      <span class="mf-store">SQLite FTS5 + Vector</span>
    </div>
    <div class="mf-body">${escapeHtml(fact)}</div>
  `;
  list.prepend(factEl);
}

function appendAuditRecord(tool, verdict, details) {
  const auditContainer = $('[data-audit-list]');
  if (!auditContainer) return;
  const empty = auditContainer.querySelector('.audit-empty');
  if (empty) empty.remove();

  let list = auditContainer.querySelector('.audit-record-list');
  if (!list) {
    list = document.createElement('div');
    list.className = 'audit-record-list';
    auditContainer.appendChild(list);
  }

  const time = new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' });
  const isAllow = verdict === 'ALLOW';
  const record = document.createElement('div');
  record.className = 'audit-record-item';
  record.innerHTML = `
    <div class="ar-head">
      <span class="ar-time">${time}</span>
      <span class="ar-tool">${escapeHtml(tool)}</span>
      <span class="ar-badge ${isAllow ? 'ar-badge--allow' : 'ar-badge--deny'}">${verdict}</span>
      <span class="ar-vault">🔒 Vault Sealed</span>
    </div>
    <div class="ar-details">${escapeHtml(details || 'Action validated against active policy rules')}</div>
  `;
  list.prepend(record);
}

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

  // Preset prompt hint
  if (composer && bot.prompt) {
    composer.placeholder = 'Try: ' + bot.prompt.slice(0, 75) + '...';
  }

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
  if (window.innerWidth <= 900 && sidebar) sidebar.classList.remove('is-open');
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
    if (bot) {
      selectBot(bot);
      if (composer && bot.prompt) {
        composer.value = bot.prompt;
        composer.focus();
      }
    }
  });
});

// === Search ===
if (searchInput) {
  searchInput.addEventListener('input', () => {
    const q = searchInput.value.toLowerCase();
    botRows.forEach(row => {
      const name = $x(row, '.product-demo__bot-name');
      const desc = $x(row, '.product-demo__bot-preview');
      row.classList.toggle('is-hidden', name && desc && !name.textContent.toLowerCase().includes(q) && !desc.textContent.toLowerCase().includes(q));
    });
  });
}

// === Messaging ===
function addMessage(type, tag, text, bot) {
  const isUser = type === 'user';
  const b = bot || activeBot;
  const time = new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  const div = document.createElement('div');
  div.className = 'pd-message pd-message--' + (isUser ? 'user' : 'bot');

  if (isUser) {
    div.innerHTML = `
      <div class="pd-message-avatar pd-message-avatar--user">FH</div>
      <div class="pd-bubble pd-bubble--user">${escapeHtml(text)}</div>
    `;
  } else {
    const tagHtml = tag ? `<span class="pd-bubble__tag">${tag} <span class="pd-bubble__time">${time}</span></span>` : '';
    const bodyContent = tag === 'done' || tag === 'plan' ? renderMarkdown(text) : escapeHtml(text);
    div.innerHTML = `
      <div class="pd-message-avatar" style="background:${b.color}">${b.icon}</div>
      <div class="pd-bubble pd-bubble--${tag || 'bot'}">
        ${tagHtml}
        <div class="pd-bubble__body">${bodyContent}</div>
      </div>
    `;
  }
  thread.appendChild(div);
  thread.scrollTop = thread.scrollHeight;
  return div;
}

function addApprovalGate(toolName, description) {
  const time = new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  const div = document.createElement('div');
  div.className = 'pd-message pd-message--bot';
  div.innerHTML = `
    <div class="pd-message-avatar" style="background:#ef4444">🛡️</div>
    <div class="pd-bubble pd-bubble--verify" style="border-left-color: #f59e0b;">
      <span class="pd-bubble__tag" style="color: #f59e0b;">GOVERNANCE GATE <span class="pd-bubble__time">${time}</span></span>
      <div class="pd-bubble__body">
        <strong>Approval Required:</strong> Agent requests execution of <code>${escapeHtml(toolName)}</code>.<br>
        <span style="color: var(--text-secondary); font-size: 12px;">${escapeHtml(description || 'Policy rule requires operator verification for this gated tool.')}</span>
        <div class="pd-gate-actions" style="display: flex; gap: 8px; margin-top: 10px;">
          <button type="button" class="pd-gate-btn pd-gate-btn--allow" data-gate-allow>✓ Allow Tool Call</button>
          <button type="button" class="pd-gate-btn pd-gate-btn--deny" data-gate-deny>✕ Deny</button>
        </div>
      </div>
    </div>
  `;
  thread.appendChild(div);
  thread.scrollTop = thread.scrollHeight;

  return new Promise((resolve) => {
    const allowBtn = div.querySelector('[data-gate-allow]');
    const denyBtn = div.querySelector('[data-gate-deny]');
    const handleDecision = (allowed) => {
      allowBtn.disabled = true;
      denyBtn.disabled = true;
      allowBtn.style.opacity = '0.5';
      denyBtn.style.opacity = '0.5';
      appendAuditRecord(toolName, allowed ? 'ALLOW' : 'DENY', allowed ? 'Manually approved by human operator' : 'Rejected by human operator');
      resolve(allowed);
    };
    if (allowBtn) allowBtn.addEventListener('click', () => handleDecision(true));
    if (denyBtn) denyBtn.addEventListener('click', () => handleDecision(false));
  });
}

function addThinking(bot) {
  const b = bot || activeBot;
  const time = new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  const div = document.createElement('div');
  div.className = 'pd-message pd-message--bot';
  div.innerHTML = `
    <div class="pd-message-avatar" style="background:${b.color}">${b.icon}</div>
    <div class="pd-bubble pd-bubble--thinking">
      <span class="pd-bubble__tag">THINKING <span class="pd-bubble__time">${time}</span></span>
      <div class="pd-bubble__body pd-stream-cursor"></div>
    </div>
  `;
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
  const content = tag === 'done' || tag === 'plan' ? renderMarkdown(text) : escapeHtml(text);
  bubble.innerHTML = `
    <span class="pd-bubble__tag">${(tag || 'BOT').toUpperCase()} <span class="pd-bubble__time">${time}</span></span>
    <div class="pd-bubble__body">${content}</div>
  `;
  thread.scrollTop = thread.scrollHeight;
}

function addTimestamp() {
  const time = new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  const div = document.createElement('div');
  div.className = 'pd-time';
  div.textContent = time;
  thread.appendChild(div);
}

function escapeHtml(s) {
  if (!s) return '';
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
    + '5. **DELIVER** — Present the final output to the user (use Markdown tables where appropriate)\n\n'
    + 'IMPORTANT: You MUST respond in a structured format. First output a PLAN section with numbered steps, then simulate tool calls with TOOL sections, then MEMORY sections for absorbed facts, then VERIFY, then FINAL.\n\n'
    + 'Use this format:\n'
    + '[PLAN]\n'
    + '1. Step one\n'
    + '2. Step two\n...\n\n'
    + '[TOOL: search_business_directory]\n'
    + 'Query: "london financial ciso"\n'
    + 'Result: Found 12 matching institutions...\n\n'
    + '[MEMORY]\n'
    + 'Absorbed: Barclays and HSBC CISO contact patterns verified via MX/SMTP.\n\n'
    + '[VERIFY]\n'
    + 'Checked: SMTP 250 OK verification on 50 addresses. Zero disposable domains.\n'
    + 'Status: PASS\n\n'
    + '[FINAL]\n'
    + '### Deliverable Summary\n'
    + '| Company | Name | Title | Email | Status |\n'
    + '|---|---|---|---|---|\n'
    + '| Barclays | Sarah Jenkins | CISO | s.jenkins@barclays.co.uk | Verified (250 OK) |\n\n'
    + 'Be thorough. Use multiple tools. Show real-looking data. The user is watching a live demo - make it impressive.';
}

// === Send message ===
async function sendMessage() {
  const text = composer.value.trim() || activeBot.prompt;
  if (!text || isRunning) return;
  composer.value = '';
  isRunning = true;
  sendBtn.classList.add('busy');
  sendBtn.innerHTML = '&#x25A0;'; // Stop icon
  runState.textContent = 'running';
  runState.className = 'product-demo__run-state running';
  dsStatus.textContent = 'busy';
  dsStatus.className = 'ds-v ds-v--busy';
  pulse.classList.add('active');
  startTime = Date.now();
  abortController = new AbortController();

  // Swarm node addition
  appendSwarmNode(activeBot.name, activeBot.role, 'running', 0, text.slice(0, 60) + '...');
  const currentRunAgents = Math.floor(Math.random() * 3) + 2;
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
        const planText = trimmed.replace('[PLAN]', '').trim();
        finalizeThinking(thinkingDiv, 'plan', planText);
        thinkingFinalized = true;
        // Spawn subagents in tree
        appendSwarmNode('Researcher-1', 'OSINT Specialist', 'running', 1, 'Data discovery & directory parsing');
        appendSwarmNode('Verifier-2', 'SMTP Validator', 'running', 1, 'Port 25 MX handshake verification');
      } else if (trimmed.startsWith('[TOOL')) {
        if (!thinkingFinalized) { finalizeThinking(thinkingDiv, 'info', trimmed); thinkingFinalized = true; }
        const tagMatch = trimmed.match(/\[TOOL:\s*([^\]]+)\]/);
        const toolName = tagMatch ? tagMatch[1] : 'tool';
        const tag = 'tool: ' + toolName;
        const toolOutput = trimmed.replace(/\[TOOL:[^\]]*\]/, '').trim();
        addMessage('bot', tag, toolOutput, activeBot);

        // Update Computer Panel URL
        if (computerUrl) computerUrl.textContent = 'fathom://' + activeBot.id + '/tool/' + toolName;
        if (computerStatus) computerStatus.textContent = 'running ' + toolName;

        // Record in Audit trail
        appendAuditRecord(toolName, 'ALLOW', 'ActionContext passed role policies (' + activeBot.role + ')');
      } else if (trimmed.startsWith('[MEMORY]')) {
        if (!thinkingFinalized) { finalizeThinking(thinkingDiv, 'info', trimmed); thinkingFinalized = true; }
        const memText = trimmed.replace('[MEMORY]', '').trim();
        addMessage('bot', 'memory', memText, activeBot);
        // Update memory count and list
        memEl = $('[data-mem-facts]');
        if (memEl) memEl.textContent = String(parseInt(memEl.textContent || '0') + 1);
        appendMemoryFact(memText, 'entity/' + activeBot.id, '0.96');
      } else if (trimmed.startsWith('[VERIFY]')) {
        if (!thinkingFinalized) { finalizeThinking(thinkingDiv, 'info', trimmed); thinkingFinalized = true; }
        addMessage('bot', 'verify', trimmed.replace('[VERIFY]', '').trim(), activeBot);
      } else if (trimmed.startsWith('[FINAL]')) {
        const finalContent = trimmed.replace('[FINAL]', '').trim();
        if (!thinkingFinalized) { finalizeThinking(thinkingDiv, 'done', finalContent); thinkingFinalized = true; }
        addMessage('bot', 'done', finalContent, activeBot);
        hasFinal = true;
      } else {
        if (hasFinal) {
          addMessage('bot', 'info', trimmed, activeBot);
        }
      }
    }

    if (!thinkingFinalized) {
      finalizeThinking(thinkingDiv, hasFinal ? 'info' : 'done', fullResponse);
    }

    // Update computer screen with mockup
    const mockup = activeBot.mockup;
    if (mockup) {
      computerBody.innerHTML = '<img src="' + mockup + '" class="pd-window-shot" alt="' + activeBot.name + ' screen" />';
    }
    computerStatus.textContent = 'task complete';

    // Update stats
    runCount++;
    runState.textContent = 'done';
    runState.className = 'product-demo__run-state done';
    dsStatus.textContent = 'idle';
    dsStatus.className = 'ds-v ds-v--ok';
    pulse.classList.remove('active');
    if (dsRuns) dsRuns.textContent = runCount;
    if (dsTokens) dsTokens.textContent = totalTokens;
    const elapsedRaw = (Date.now() - startTime) / 1000;
    const mins = Math.floor(elapsedRaw / 60);
    const secs = Math.floor(elapsedRaw % 60);
    if (dsLatency) dsLatency.textContent = String(mins).padStart(2, '0') + ':' + String(secs).padStart(2, '0');

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

    // Audit counts
    const allowed = $('[data-audit-allowed]');
    if (allowed) allowed.textContent = String(parseInt(allowed.textContent || '0') + toolCount);
    const memGc = $('[data-mem-gc]');
    if (memGc && memGc.textContent === '—') memGc.textContent = '142s to next GC';
    const memConf = $('[data-mem-confidence]');
    if (memConf && memConf.textContent === '—') memConf.textContent = '0.94';

  } catch (err) {
    pulse.classList.remove('active');
    const failed = $('[data-fleet-failed]');
    if (failed) failed.textContent = String(parseInt(failed.textContent || '0') + currentRunAgents);
    if (err.name === 'AbortError') {
      finalizeThinking(thinkingDiv, 'info', 'Execution aborted by user.');
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
    sendBtn.innerHTML = '&#x2191;';
    abortController = null;
  }
}

// === Event listeners ===
sendBtn.addEventListener('click', () => {
  if (isRunning && abortController) {
    abortController.abort();
  } else {
    sendMessage();
  }
});

composer.addEventListener('keydown', function(e) {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault();
    sendMessage();
  }
});

// === Panel toggle ===
if (panelToggle) {
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
}

if (panelClose) {
  panelClose.addEventListener('click', function() {
    computerPanel.style.display = 'none';
    if (panelToggle) {
      panelToggle.setAttribute('aria-pressed', 'false');
      panelToggle.style.color = 'var(--text-tertiary)';
    }
  });
}

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
if (screenTake) screenTake.addEventListener('click', openLightbox);
if (screenTakeSm) screenTakeSm.addEventListener('click', openLightbox);

function openLightbox() {
  const mockup = activeBot.mockup;
  if (mockup) {
    lightboxImg.src = mockup;
    lightboxUrl.textContent = 'fathom://' + activeBot.id + '/screen';
    lightboxCaption.textContent = activeBot.name + ' — live desktop session';
    lightbox.removeAttribute('hidden');
  }
}

if (lightboxClose) {
  lightboxClose.addEventListener('click', function() {
    lightbox.setAttribute('hidden', '');
  });
}

if (lightbox) {
  lightbox.addEventListener('click', function(e) {
    if (e.target === lightbox) lightbox.setAttribute('hidden', '');
  });
}

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

// === Handle Escape ===
document.addEventListener('keydown', function(e) {
  if (e.key === 'Escape') {
    if (sidebar) sidebar.classList.remove('is-open');
    if (computerPanel) {
      computerPanel.style.display = '';
      computerPanel.classList.remove('is-open');
    }
    if (lightbox) lightbox.setAttribute('hidden', '');
  }
});

// === Initial active state ===
selectBot(activeBot);
