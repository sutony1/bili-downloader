const assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
const source=fs.readFileSync('src/extract.js','utf8');
const bv='BV1234567890';
async function run(overrides={}) {
 const messages=[];
 const context={expectedBv:bv,requestId:17,location:{pathname:'/video/'+bv+'/'},document:{readyState:'complete',querySelector:()=>null},window:{ipc:{postMessage:s=>messages.push(JSON.parse(s))}},performance:{getEntriesByType:()=>[]},URL,AbortController,setTimeout,clearTimeout,...overrides};
 await vm.runInNewContext(source,context);return messages;
}
(async()=>{
 const waiting=await run({location:{pathname:'/'}});assert.equal(waiting.at(-1).waiting,true);assert.equal(waiting.at(-1).bv,bv);assert.equal(waiting.at(-1).requestId,17);
 const loading=await run({document:{readyState:'loading'}});assert.equal(loading.at(-1).waiting,true);
 let call=0;
 const success=await run({fetch:async()=>({ok:true,json:async()=>++call===1?{code:0,data:{cid:1,title:'Test',duration:60,pages:[{}]}}:{code:0,data:{dash:{video:[{id:64,width:1280,height:720,baseUrl:'https://cdn.bilivideo.com/v720'},{id:80,width:1920,height:1080,baseUrl:'https://cdn.bilivideo.com/v1080'}],audio:[{bandwidth:10,baseUrl:'https://node.mcdn.bilivideo.cn/a',backupUrl:['https://cdn.bilivideo.com/a']}]}}}})});
 assert.equal(success[0].started,true);assert.equal(success.at(-1).width,1920);assert.equal(success.at(-1).requestId,17);assert.equal(success.at(-1).audio,"https://cdn.bilivideo.com/a");
 const timeout=await run({setTimeout:(fn,ms)=>setTimeout(fn,Math.min(ms,5)),fetch:async(_,options)=>new Promise((_,reject)=>options.signal.addEventListener('abort',()=>reject(Object.assign(Error('abort'),{name:'AbortError'}))))});
 assert.match(timeout.at(-1).error,/超时/);
 const fallback=await run({fetch:async()=>{throw Error('network');},performance:{getEntriesByType:()=>[{name:'https://cdn.bilivideo.com/clip-100.m4s'},{name:'https://node.mcdn.bilivideo.cn/clip-30280.m4s'}]},document:{readyState:'complete',querySelector:s=>s==='video'?{duration:25,videoWidth:480,videoHeight:852}:{textContent:'Fallback'}}});
 assert.equal(fallback.at(-1).width,480);assert.match(fallback.at(-1).audio,/bilivideo.cn/);assert.equal(fallback.at(-1).duration,25);
 console.log('PASS: early-page wait, page-loading wait, highest-quality extraction, timed-out API error, playback-resource fallback');
})().catch(e=>{console.error(e);process.exitCode=1;});
