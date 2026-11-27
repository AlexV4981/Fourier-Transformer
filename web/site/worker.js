import init,{from_gray,from_image,equation,paste_payload,html,clear} from './pkg/fourier_web.js';
try{await init();postMessage({kind:'ready'});}catch(e){postMessage({kind:'error',operation:'init',message:String(e)});}
self.onmessage=({data:m})=>{
 try{
  if(m.kind==='calculate'){clear();const value=m.mode==='image'?from_image(new Uint8Array(m.bytes),m.detail,m.cap,m.traceMode,m.sensitivity):from_gray(m.w,m.h,new Uint8Array(m.bytes),m.detail,m.cap,true);postMessage({kind:'drawing',value:JSON.parse(value)});}
  else if(m.kind==='equation')postMessage({kind:'equation',id:m.id,value:equation(m.index),payload:JSON.parse(paste_payload(m.index))});
  else if(m.kind==='export')postMessage({kind:'export',value:html()});
 }catch(e){postMessage({kind:'error',operation:m.kind,id:m.id,message:String(e)});}
};