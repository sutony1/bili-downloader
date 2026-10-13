(() => {
 const send = x => window.ipc.postMessage(JSON.stringify({ platform: 'douyin', requestId, ...x }));
 const pathId = location.pathname.match(/\/(?:video|note)\/(\d{15,22})(?:\/|$)/)?.[1];
 const id = expectedId || pathId;
 if (!id || pathId !== id || document.readyState === 'loading') { send({ waiting: true }); return; }
 const capture = window.__douyinCapture;
 if (capture) {
  for (const key of ['_ROUTER_DATA','__INITIAL_STATE__','__NEXT_DATA__','RENDER_DATA']) { try { capture.ingest(window[key]); } catch {} }
  for (const element of document.querySelectorAll('script[type="application/json"],script#RENDER_DATA,script#__NEXT_DATA__')) { try { const raw = element.textContent; capture.ingest(JSON.parse(raw.startsWith('%') ? decodeURIComponent(raw) : raw)); } catch {} }
  for (const start of document.querySelectorAll('video,[data-e2e="video-detail"],[data-e2e="feed-active-video"]')) {
   for (let element = start, parents = 0; element && parents < 10; element = element.parentElement, parents++) {
    const key = Object.keys(element).find(k => k.startsWith('__reactFiber') || k.startsWith('__reactInternalInstance')); let fiber = key && element[key];
    for (let depth = 0; fiber && depth < 35; depth++, fiber = fiber.return) { capture.ingest(fiber.memoizedProps); capture.ingest(fiber.memoizedState); }
   }
  }
 }
 const detail = capture?.items.get(id), video = detail?.video, formats = [], urls = new Set();
 const hostAllowed = h => ['douyinvod.com','douyin.com','bytecdn.cn','bytedance.com','ibytedtos.com','amemv.com','ixigua.com','snssdk.com'].some(d => h === d || h.endsWith('.' + d));
 function add(address, bitrate = 0, label = '', width = 0, height = 0) {
  if (!address) return;
  for (const raw of address.url_list || address.urlList || address.UrlList || []) { try { const u = new URL(raw.startsWith('//') ? 'https:' + raw : raw); if (u.protocol === 'http:') u.protocol = 'https:'; if (u.protocol !== 'https:' || !hostAllowed(u.hostname) || urls.has(u.href)) continue; urls.add(u.href); const w = Number(address.width || address.Width || width || video?.width || 0), h = Number(address.height || address.Height || height || video?.height || 0); formats.push({ id: String(formats.length), url: u.href, width: w, height: h, bitrate: Number(bitrate) || 0, label: `${w && h ? w + '×' + h : '网页可用画质'}${bitrate ? ' · ' + (Number(bitrate) / 1000000).toFixed(1) + ' Mbps' : ''}${label ? ' · ' + label : ''}` }); break; } catch {} }
 }
 if (video) {
  if (detail.prevent_download === true || detail.prevent_download === 1 || detail.status?.allow_download === false) { send({ id, error: '该作品明确禁止下载，请选择允许下载的作品。' }); return; }
  for (const entry of video.bit_rate || video.bitRate || video.bitrateInfo || []) { if (entry.is_bytevc2 || /bytevc2|h266|vvc/i.test(entry.gear_name || entry.GearName || '')) continue; add(entry.play_addr || entry.playAddr || entry.PlayAddr, entry.bit_rate || entry.bitRate || entry.Bitrate, '', entry.width, entry.height); }
  add(video.play_addr || video.playAddr, 0, '默认播放版本'); add(video.play_addr_h264, 0, 'H.264'); if (!formats.length) add(video.download_addr || video.downloadAddr, 0, '下载版本');
 }
 const player = [...document.querySelectorAll('video')].find(x => Number.isFinite(x.duration) && x.duration > 0 && x.videoWidth > 0);
 let playbackFallback = false;
 if (!formats.length && player?.currentSrc && !player.currentSrc.startsWith('blob:')) { add({ url_list: [player.currentSrc], width: player.videoWidth, height: player.videoHeight }, 0, '当前播放画质'); playbackFallback = true; }
 if (!formats.length) { send({ id, waiting: true }); return; }
 formats.sort((a,b) => b.width * b.height - a.width * a.height || b.bitrate - a.bitrate);
 const distinct = formats.filter((f,i,a) => !a.slice(0,i).some(x => x.width === f.width && x.height === f.height && x.bitrate === f.bitrate)); distinct.forEach((f,i) => f.id = String(i));
 const duration = Number(video?.duration) / 1000 || player?.duration; if (!Number.isFinite(duration) || duration <= 0) { send({ id, waiting: true }); return; }
 send({ id, title: detail?.desc || document.title.replace(/[-_ ]*抖音.*$/, '') || '抖音视频', duration, formats: distinct, source: playbackFallback ? 'playback' : 'metadata', userAgent: navigator.userAgent });
})();
