const $=id=>document.getElementById(id), canvas=$('plot'), ctx=canvas.getContext('2d');
let drawing=[], curves=[], bounds=null, position=0, playing=false, last=0, req=0, full='', paste='', upperBound='', busy=false, animationId=0;
let worker, pending=null, workerUsed=false, workerReady=false;
const controls=['play','reset','equation','copy','export','finish'];
function enabled(v){for(const id of controls)$(id).disabled=!v;}
function status(s){$('status').textContent=s;}
function fail(e){busy=false;pending=null;$('calculate').disabled=false;status(String(e));}
function startWorker(){
 if(worker)worker.terminate();
 workerReady=false;
 const current=new Worker(new URL('./worker.js',import.meta.url),{type:'module'});
 worker=current;
 current.onerror=e=>{if(current!==worker)return;workerReady=false;fail(e.message);};
 current.onmessage=e=>{if(current===worker)handleMessage(e);};
}
function sendCalculation(message, transfer){
 // WASM linear memory cannot shrink. Retire the previous worker on recalculation.
 if(workerUsed||!workerReady){pending={message,transfer};startWorker();}else{workerUsed=true;worker.postMessage(message,transfer);}
}
function requestEquation(){full='';paste='';upperBound='';$('copy').disabled=true;$('copyBound').disabled=true;$('output').value='Loading…';worker.postMessage({kind:'equation',index:Number($('equation').value),id:++req});}
function handleMessage({data:m}){
 if(m.kind==='ready'){workerReady=true;if(pending){const p=pending;pending=null;workerUsed=true;worker.postMessage(p.message,p.transfer);}else{$('calculate').disabled=false;status('Ready');}}
 if(m.kind==='error'){if(m.operation==='init')workerReady=false;if(m.operation==='equation'&&m.id!==req)return;if(m.operation==='equation'){$('output').value=m.message;$('copy').disabled=true;status(m.message);return;}if(m.operation==='export'){status(m.message);return;}fail(m.message);}
 if(m.kind==='drawing'){
  busy=false;drawing=m.value.paths;position=0;playing=false;$('play').textContent='Play';
  curves=drawing.map(p=>{
    const n=Math.min(4096,Math.max(512,4*Math.max(0,...p.coeff.map(c=>Math.abs(c[0])))));
    return Array.from({length:n+1},(_,i)=>point(p,i/n));
  });
  let minx=Infinity,maxx=-Infinity,miny=Infinity,maxy=-Infinity;
  for(const p of drawing)for(const [x,y] of p.preview){minx=Math.min(minx,x);maxx=Math.max(maxx,x);miny=Math.min(miny,y);maxy=Math.max(maxy,y);}
  bounds=[minx,maxx,miny,maxy];const n=drawing.reduce((s,p)=>s+p.coeff.length,0);
  $('stats').textContent=drawing.length+' paths · '+n+' coefficients · '+Math.round(n*12/1024)+' KiB coefficient payload'+(m.value.candidates?' · '+m.value.candidates+' candidate paths':'')+(m.value.subjectDetected?' · foreground prioritized':'')+(m.value.limited?' · some detail omitted by selected limits':'');
  $('equation').replaceChildren(new Option('Instructions',0),...drawing.map((p,i)=>new Option('Path '+(i+1)+' ('+p.name+')',i+1)),new Option('Master Pen',drawing.length+1));
  enabled(true);$('calculate').disabled=false;status('Ready');requestEquation();render();
 }
 if(m.kind==='equation'&&m.id===req){
   full=m.value;paste=m.payload.expression;upperBound=m.payload.bound;
   // Keep the parameter bounds visible even if the equation is very long.
   const marker=full.lastIndexOf('\n\nBounds:');
   const body=marker>=0?full.slice(0,marker):full;
   const tail=marker>=0?full.slice(marker):'';
   $('output').value=body.length>4000?body.slice(0,4000)+'\n[Preview truncated; copy includes full equation]'+tail:full;
   $('copy').disabled=false;$('copyBound').disabled=!upperBound;
 }
 if(m.kind==='export'&&!busy&&drawing.length){const url=URL.createObjectURL(new Blob([m.value],{type:'text/html'}));const a=document.createElement('a');a.href=url;a.download='fourier-desmos.html';a.click();setTimeout(()=>URL.revokeObjectURL(url),1000);status('Desmos HTML exported.');}
};
function point(p,t){let x=0,y=0;for(const [f,r,a] of p.coeff){const q=2*Math.PI*f*t+a;x+=r*Math.cos(q);y+=r*Math.sin(q);}return [x,y];}
function project([x,y]){const [a,b,c,d]=bounds,s=.9*Math.min(canvas.width/Math.max(b-a,.01),canvas.height/Math.max(d-c,.01));return [canvas.width/2+(x-(a+b)/2)*s,canvas.height/2-(y-(c+d)/2)*s];}
function line(points,color,width=1.5){if(points.length<2)return;ctx.beginPath();for(let i=0;i<points.length;i++){const [x,y]=project(points[i]);if(i===0)ctx.moveTo(x,y);else ctx.lineTo(x,y);}ctx.strokeStyle=color;ctx.lineWidth=width;ctx.stroke();}
function render(){
 ctx.fillStyle='#fff';ctx.fillRect(0,0,canvas.width,canvas.height);if(!bounds||!drawing.length)return;
 if($('original').checked)for(const p of drawing)line(p.preview,'#d1d5db');
 const i=Math.min(drawing.length-1,Math.floor(position)),t=Math.min(1,position-i);
 for(let j=0;j<i;j++)line(curves[j],$('ink').checked?drawing[j].color:'#2563eb',1.6);
 line([...curves[i].slice(0,Math.floor(t*(curves[i].length-1))+1),point(drawing[i],t)],$('ink').checked?drawing[i].color:'#2563eb',1.6);
 if($('circles').checked&&position<drawing.length){let x=0,y=0;for(const [f,r,a] of drawing[i].coeff){const q=2*Math.PI*f*t+a, p=project([x,y]);const nx=x+r*Math.cos(q),ny=y+r*Math.sin(q),e=project([nx,ny]);const edge=project([x+r,y]);ctx.beginPath();ctx.arc(p[0],p[1],Math.abs(edge[0]-p[0]),0,Math.PI*2);ctx.strokeStyle='#49647b';ctx.lineWidth=.7;ctx.stroke();ctx.beginPath();ctx.moveTo(...p);ctx.lineTo(...e);ctx.strokeStyle='#fbbf24';ctx.stroke();x=nx;y=ny;}const p=project([x,y]);ctx.beginPath();ctx.arc(...p,3,0,Math.PI*2);ctx.fillStyle='#f87171';ctx.fill();}
}
function frame(now,id){if(!playing||id!==animationId)return;position=Math.min(drawing.length,position+Math.min((now-last)/1000,.1)*Number($('speed').value));last=now;render();if(position<drawing.length)requestAnimationFrame(next=>frame(next,id));else{playing=false;$('play').textContent='Play';}}
$('play').onclick=()=>{if(playing){playing=false;animationId++;$('play').textContent='Play';return;}if(position>=drawing.length)position=0;playing=true;last=performance.now();$('play').textContent='Pause';const id=++animationId;requestAnimationFrame(now=>frame(now,id));};
$('reset').onclick=()=>{playing=false;animationId++;position=0;$('play').textContent='Play';render();};
$('finish').onclick=()=>{playing=false;animationId++;position=drawing.length;$('play').textContent='Play';render();};
for(const id of ['circles','original','ink'])$(id).onchange=render;
$('mode').onchange=()=>{const image=$('mode').value==='image';$('imageGroup').hidden=!image;$('colorGroup').hidden=!image;$('textGroup').hidden=image;$('detail').value=image?300:150;$('detailValue').value=$('detail').value;};
$('trace').onchange=()=>{
 const trace=$('trace').value;
 $('sensitivityGroup').hidden=trace==='gray';
 $('cap').max=trace==='high'?400:200;
 if(trace==='high'){$('cap').value=400;$('detail').value=500;$('sensitivity').value=65;}
 else if(trace==='subject'){$('cap').value=150;$('detail').value=300;$('sensitivity').value=45;}
 for(const id of ['cap','detail','sensitivity'])$(id+'Value').value=$(id).value;
};
for(const id of ['detail','cap','sensitivity'])$(id).oninput=()=>$(id+'Value').value=$(id).value;
$('calculate').onclick=async()=>{
 if(busy)return;busy=true;playing=false;animationId++;req++;drawing=[];curves=[];bounds=null;render();enabled(false);$('copyBound').disabled=true;full='';paste='';upperBound='';$('output').value='';$('calculate').disabled=true;status('Calculating in Rust…');
 try{
  const mode=$('mode').value,detail=Number($('detail').value),cap=Number($('cap').value);
  if(mode==='image'){const f=$('file').files[0];if(!f)throw Error('Choose an image');if(f.size>16*1024*1024)throw Error('Maximum file size 16 MiB');const bytes=await f.arrayBuffer();sendCalculation({kind:'calculate',mode,detail,cap,bytes,traceMode:({gray:0,color:1,subject:2,high:3})[$('trace').value],sensitivity:Number($('sensitivity').value)},[bytes]);}
  else{
   const text=$('text').value.trim().replaceAll('[heart]','♥').replaceAll('<3','♥').replaceAll('[square]','■').replaceAll('[star]','★').replaceAll('[note]','♫');if(!text)throw Error('Enter some text');
   const c=document.createElement('canvas'),g=c.getContext('2d',{willReadFrequently:true});g.font='100px Arial, sans-serif';const width=g.measureText(text).width;const font=Math.min(100,3600/Math.max(width,1)*100);g.font=font+'px Arial, sans-serif';c.width=Math.ceil(g.measureText(text).width)+32;c.height=Math.ceil(font*1.7)+32;g.fillStyle='white';g.fillRect(0,0,c.width,c.height);g.font=font+'px Arial, sans-serif';g.textBaseline='middle';g.fillStyle='black';g.fillText(text,16,c.height/2);const rgba=g.getImageData(0,0,c.width,c.height).data;const gray=new Uint8Array(c.width*c.height);for(let i=0;i<gray.length;i++)gray[i]=rgba[i*4];sendCalculation({kind:'calculate',mode,detail,cap,w:c.width,h:c.height,bytes:gray.buffer},[gray.buffer]);
  }
 }catch(e){fail(e);}
};
$('equation').onchange=requestEquation;
$('copy').onclick=async()=>{const selected=req;try{await navigator.clipboard.writeText(paste);if(selected===req&&!busy)status('Copied expression only');}catch(e){if(selected===req&&!busy)status('Clipboard blocked: '+e.message);}};
$('copyBound').onclick=async()=>{try{await navigator.clipboard.writeText(upperBound);status('Copied upper t bound');}catch(e){status('Clipboard blocked: '+e.message);}};
$('export').onclick=()=>worker.postMessage({kind:'export'});

startWorker();
