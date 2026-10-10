(async () => {
 const bv=expectedBv;
 const send=value=>window.ipc.postMessage(JSON.stringify({bv,requestId,...value}));
 if(location.pathname.match(/BV[0-9A-Za-z]{10}/)?.[0]!==bv||document.readyState==='loading'){send({waiting:true});return;}
 send({started:true});
 const mediaHost=h=>h==='bilivideo.com'||h.endsWith('.bilivideo.com')||h==='bilivideo.cn'||h.endsWith('.bilivideo.cn');
 const mediaUrl=stream=>{
  const urls=[stream.baseUrl||stream.base_url,...(stream.backupUrl||stream.backup_url||[])].flatMap(raw=>{
   try{const u=new URL(raw);if(!mediaHost(u.hostname))return [];if(u.protocol==='http:')u.protocol='https:';return u.protocol==='https:'?[u.href]:[];}catch{return [];}
  });
  // Prefer the conventional CDN over peer/cache nodes when Bili offers both.
  return urls.find(raw=>new URL(raw).hostname.endsWith('.bilivideo.com'))||urls[0]||'';
 };
 const getJson=async url=>{const controller=new AbortController();const timer=setTimeout(()=>controller.abort(),7000);try{const r=await fetch(url,{credentials:'include',signal:controller.signal});if(!r.ok)throw Error('HTTP '+r.status);return await r.json();}finally{clearTimeout(timer);}};
 try{
  const view=await getJson('https://api.bilibili.com/x/web-interface/view?bvid='+bv);
  if(view.code!==0)throw Error('视频信息接口：'+view.code);
  const d=view.data;if(d.pages?.length>1)throw Error('目前仅支持单P视频');
  const play=await getJson('https://api.bilibili.com/x/player/playurl?bvid='+bv+'&cid='+d.cid+'&qn=127&fnval=4048&fourk=1');
  if(play.code!==0||!play.data?.dash?.video?.length)throw Error('播放接口暂不可用');
  const p=play.data;const v=[...p.dash.video].sort((a,b)=>(b.id-a.id)||(b.width*b.height-a.width*a.height)||(parseFloat(b.frameRate||b.frame_rate)-parseFloat(a.frameRate||a.frame_rate))||(b.bandwidth-a.bandwidth))[0];
  const a=[...(p.dash.audio||[])].sort((a,b)=>b.bandwidth-a.bandwidth)[0];if(!a)throw Error('未找到音频');
  send({title:d.title,duration:d.duration,video:mediaUrl(v),audio:mediaUrl(a),quality:p.support_formats?.find(x=>x.quality===v.id)?.new_description||p.support_formats?.find(x=>x.quality===v.id)?.display_desc||String(v.id),width:v.width,height:v.height});
 }catch(err){
  const perf=performance.getEntriesByType('resource').map(x=>x.name).filter(x=>{try{const u=new URL(x);return u.protocol==='https:'&&mediaHost(u.hostname)&&u.pathname.endsWith('.m4s')}catch{return false}});
  const vids=perf.filter(x=>!/-302\d+\.m4s/.test(x)),auds=perf.filter(x=>/-302\d+\.m4s/.test(x));const player=document.querySelector('video');
  if(vids.length&&auds.length&&Number.isFinite(player?.duration)&&player.duration>0){send({title:document.querySelector('h1')?.textContent||bv,duration:player.duration,video:vids.at(-1),audio:auds.at(-1),quality:'当前播放画质（请在播放器确认）',width:player.videoWidth,height:player.videoHeight});}
  else send({error:'解析失败：'+(err.name==='AbortError'?'接口请求超时':err.message)+'。请检查网络，或显示浏览器确认视频能够加载后重试。'});
 }
})();
