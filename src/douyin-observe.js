(() => {
 if (location.protocol !== 'https:' || !(location.hostname === 'douyin.com' || location.hostname.endsWith('.douyin.com'))) return;
 const items = new Map();
 function ingest(root) {
  const seen = new WeakSet(); let budget = 18000;
  function walk(v, depth) {
   if (!v || typeof v !== 'object' || seen.has(v) || depth > 10 || --budget < 0) return;
   seen.add(v); const id = String(v.aweme_id || v.awemeId || v.id || '');
   if (/^\d{15,22}$/.test(id) && v.video && typeof v.video === 'object') { items.set(id, v); if (items.size > 20) items.delete(items.keys().next().value); return; }
   for (const key of Object.keys(v)) { if (!['return','child','sibling','stateNode','_owner','alternate'].includes(key)) { try { walk(v[key], depth + 1); } catch {} } }
  } walk(root, 0);
 }
 window.__douyinCapture = { items, ingest };
 const relevant = raw => { try { const u = new URL(raw, location.href); return (u.hostname === 'douyin.com' || u.hostname.endsWith('.douyin.com')) && u.pathname.startsWith('/aweme/v1/web/'); } catch { return false; } };
 // Observe the site's own responses; never synthesize API signatures.
 const originalFetch = window.fetch;
 window.fetch = async function (...args) { const response = await originalFetch.apply(this, args); if (relevant(typeof args[0] === 'string' ? args[0] : args[0]?.url)) response.clone().json().then(ingest).catch(() => {}); return response; };
 const originalOpen = XMLHttpRequest.prototype.open;
 XMLHttpRequest.prototype.open = function (method, url, ...rest) { if (relevant(url)) this.addEventListener('load', () => { try { ingest(this.responseType === 'json' ? this.response : JSON.parse(this.responseText)); } catch {} }, { once: true }); return originalOpen.call(this, method, url, ...rest); };
})();
