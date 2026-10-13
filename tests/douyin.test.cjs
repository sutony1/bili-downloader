const assert = require('node:assert/strict'), fs = require('node:fs'), vm = require('node:vm');
const observe = fs.readFileSync('src/douyin-observe.js', 'utf8'), extract = fs.readFileSync('src/douyin-extract.js', 'utf8');
const id = '7000000000000000001';
function context() {
 const messages = [], requests = [];
 class XHR { open(...args) { requests.push(args); } addEventListener() {} }
 const ctx = { window: { ipc: { postMessage: raw => messages.push(JSON.parse(raw)) }, fetch: async () => ({ clone: () => ({ json: async () => ({}) }) }) }, XMLHttpRequest: XHR, URL, navigator: { userAgent: 'TestBrowser' }, expectedId: id, requestId: 5, location: { href: 'https://www.douyin.com/video/' + id, protocol: 'https:', hostname: 'www.douyin.com', pathname: '/video/' + id }, document: { readyState: 'complete', title: 'Test', querySelectorAll: () => [] } };
 vm.createContext(ctx); vm.runInContext(observe, ctx);
 return { ctx, messages, run: () => { vm.runInContext(extract, ctx); return messages.at(-1); } };
}
const address = (url, w, h) => ({ url_list: [url], width: w, height: h });
const detail = { aweme_id: id, desc: 'Test', video: { duration: 12000, width: 720, height: 1280, bit_rate: [
 { bit_rate: 1000000, play_addr: address('https://v3.douyinvod.com/low', 540, 960) },
 { bit_rate: 2500000, play_addr: address('https://v3.douyinvod.com/high', 720, 1280) },
 { bit_rate: 9000000, play_addr: address('https://douyinvod.com.evil.org/fake', 1080, 1920) }
] } };
(async () => {
 const first = context(); first.ctx.window.__douyinCapture.ingest({ aweme_detail: detail }); const result = first.run();
 assert.equal(result.id, id); assert.equal(result.requestId, 5); assert.equal(result.duration, 12); assert.equal(result.formats.length, 2); assert.equal(result.formats[0].height, 1280); assert.equal(result.formats[0].id, '0');
 const wrong = context(); wrong.ctx.window.__douyinCapture.ingest({ ...detail, aweme_id: '7000000000000000002' }); assert.equal(wrong.run().waiting, true);
 const navigated = context(); navigated.ctx.location.pathname = '/video/7000000000000000002'; navigated.ctx.window.__douyinCapture.ingest(detail); assert.equal(navigated.run().waiting, true);
 const denied = context(); denied.ctx.window.__douyinCapture.ingest({ ...detail, prevent_download: true }); assert.match(denied.run().error, /禁止下载/);
 const player = context(); player.ctx.document.querySelectorAll = s => s === 'video' ? [{ duration: 12, videoWidth: 720, videoHeight: 1280, currentSrc: 'https://v3.douyinvod.com/current' }] : [];
 assert.equal(player.run().formats[0].url, 'https://v3.douyinvod.com/current');
 const blob = context(); blob.ctx.document.querySelectorAll = s => s === 'video' ? [{ duration: 12, videoWidth: 720, videoHeight: 1280, currentSrc: 'blob:https://www.douyin.com/id' }] : [];
 assert.equal(blob.run().waiting, true);
 const hydration = context(); hydration.ctx.document.querySelectorAll = s => s.startsWith('script') ? [{ textContent: encodeURIComponent(JSON.stringify({ data: detail })) }] : [];
 assert.equal(hydration.run().formats.length, 2);
 const router = context(); router.ctx.window._ROUTER_DATA = { loaderData: { page: { videoInfoRes: { aweme_detail: detail } } } }; assert.equal(router.run().formats.length, 2);
 const watched = context(); watched.ctx.window.fetch = async () => ({ clone: () => ({ json: async () => ({ aweme_detail: detail }) }) }); vm.runInContext(observe, watched.ctx); await watched.ctx.window.fetch('https://www.douyin.com/aweme/v1/web/aweme/detail/'); await new Promise(resolve => setImmediate(resolve)); assert.equal(watched.run().id, id);
 console.log('PASS: Douyin quality selection, request correlation, prefetch isolation, URL domain boundaries, permissions, hydration, playback fallback and browser-response capture');
})().catch(error => { console.error(error); process.exitCode = 1; });
