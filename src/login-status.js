(async()=>{
 if(location.protocol!=="https:" || !(location.hostname==="bilibili.com"||location.hostname.endsWith(".bilibili.com")))return;
 const send=v=>window.ipc.postMessage(JSON.stringify({action:"auth-status",...v}));
 try{
  const response=await fetch("https://api.bilibili.com/x/web-interface/nav",{credentials:"include",signal:AbortSignal.timeout(8000)});
  if(!response.ok)throw Error("request");
  const result=await response.json();
  if(result.code!==0 && result.code!==-101)throw Error("response");
  send({state:result.data?.isLogin===true?"logged-in":"logged-out",blank:!document.body||document.body.innerText.trim().length<20});
 }catch{send({state:"error",blank:false});}
})();
