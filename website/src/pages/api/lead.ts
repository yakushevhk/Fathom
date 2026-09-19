import type { APIRoute } from 'astro';

export const prerender = false;

const SCENARIOS = new Set(['outreach', 'research', 'code', 'ops', 'other']);

// Vercel/Node: process.env; Astro dev: import.meta.env
const env = (k: string) => process.env[k] || (import.meta.env as Record<string, string>)[k];

async function notifyTelegram(text: string): Promise<boolean> {
  const token = env('LEAD_TELEGRAM_BOT_TOKEN');
  const chatId = env('LEAD_TELEGRAM_CHAT_ID');
  if (!token || !chatId) return false;
  try {
    const res = await fetch(`https://api.telegram.org/bot${token}/sendMessage`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ chat_id: chatId, text, parse_mode: 'HTML', disable_web_page_preview: true }),
    });
    return res.ok;
  } catch {
    return false;
  }
}

async function notifyWebhook(payload: Record<string, unknown>): Promise<boolean> {
  const url = env('LEAD_WEBHOOK_URL');
  if (!url) return false;
  try {
    const res = await fetch(url, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    });
    return res.ok;
  } catch {
    return false;
  }
}

export const POST: APIRoute = async ({ request }) => {
  try {
    const data = await request.json();
    const email = typeof data?.email === 'string' ? data.email.trim() : '';
    const name = typeof data?.name === 'string' ? data.name.trim().slice(0, 200) : '';
    const scenario = typeof data?.scenario === 'string' && SCENARIOS.has(data.scenario) ? data.scenario : 'other';

    if (!email || !email.includes('@') || email.length > 320) {
      return new Response(JSON.stringify({ error: 'Valid email is required' }), {
        status: 400,
        headers: { 'Content-Type': 'application/json' },
      });
    }

    const payload = { email, name, scenario, timestamp: new Date().toISOString() };
    const text =
      `<b>New Fathom lead</b>\n` +
      `Email: ${email.replace(/&/g, '&amp;').replace(/</g, '&lt;')}\n` +
      `Name/TG: ${name.replace(/&/g, '&amp;').replace(/</g, '&lt;') || '—'}\n` +
      `Scenario: ${scenario}`;

    const delivered = (await Promise.all([notifyTelegram(text), notifyWebhook(payload)])).some(Boolean);
    if (!delivered) console.log('[LEAD — no sink configured]', payload);

    return new Response(
      JSON.stringify({ ok: true, message: 'Lead received successfully' }),
      { status: 200, headers: { 'Content-Type': 'application/json' } }
    );
  } catch (err: any) {
    return new Response(JSON.stringify({ error: err?.message || 'Invalid payload' }), {
      status: 400,
      headers: { 'Content-Type': 'application/json' },
    });
  }
};
