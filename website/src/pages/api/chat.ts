import type { APIRoute } from 'astro';

const API_BASE = process.env.CHAT_API_BASE || 'https://router.y7.hk/v1';
const API_KEY = process.env.CHAT_API_KEY || '';

export const prerender = false;

export const POST: APIRoute = async ({ request }) => {
  if (!API_KEY) {
    return new Response(JSON.stringify({ error: 'Chat API not configured' }), {
      status: 503,
      headers: { 'Content-Type': 'application/json' },
    });
  }
  const body = await request.text();

  const upstream = await fetch(`${API_BASE}/chat/completions`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${API_KEY}`,
    },
    body,
  });

  return new Response(upstream.body, {
    status: upstream.status,
    statusText: upstream.statusText,
    headers: {
      'Content-Type': upstream.headers.get('Content-Type') || 'text/event-stream',
    },
  });
};

export const OPTIONS: APIRoute = async () => {
  return new Response(null, {
    status: 204,
    headers: {
      'Access-Control-Allow-Origin': '*',
      'Access-Control-Allow-Methods': 'POST, OPTIONS',
      'Access-Control-Allow-Headers': 'Content-Type',
    },
  });
};