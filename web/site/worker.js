import init,{from_gray,from_image,equation,html,clear} from './pkg/fourier_web.js';
try{await init();postMessage({kind:'ready'});}catch(e){postMessage({kind:'error',message:String(e)});}
self.onmessage=({data:m})=>{
 try{
  if(m.kind==='calculate'){clear();const value=m.mode==='image'?from_image(new Uint8Array(m.bytes),m.detail,m.cap):from_gray(m.w,m.h,new Uint8Array(m.bytes),m.detail,m.cap,true);postMessage({kind:'drawing',value:JSON.parse(value)});}
  else if(m.kind==='equation')postMessage({kind:'equation',id:m.id,value:equation(m.index)});
  else if(m.kind==='export')postMessage({kind:'export',value:html()});
 }catch(e){postMessage({kind:'error',message:String(e)});}
};