const $=id=>document.getElementById(id), canvas=$('plot'), ctx=canvas.getContext('2d');
let drawing=[], curves=[], bounds=null, position=0, playing=false, last=0, req=0, full='', busy=false, animationId=0;
let worker, pending=null, workerUsed=false;
const controls=['play','reset','equation','copy','export','finish'];
function enabled(v){for(const id of controls)$(id).disabled=!v;}
function status(s){$('status').textContent=s;}
function fail(e){busy=false;$('calculate').disabled=false;status(String(e));}
function startWorker(){
 if(worker)worker.terminate();
 worker=new Worker(new URL('./worker.js',import.meta.url),{type:'module'});
 worker.onerror=e=>{pending=null;fail(e.message);};
 worker.onmessage=handleMessage;
}
function sendCalculation(message, transfer){
 // WASM linear memory cannot shrink. Retire the previous worker on recalculation.
 if(workerUsed){pending={message,transfer};startWorker();}else{workerUsed=true;worker.postMessage(message,transfer);}
}
function requestEquation(){full='';$('copy').disabled=true;$('output').value='Loading…';worker.postMessage({kind:'equation',index:Number($('equation').value),id:++req});}
function handleMessage({data:m}){
 if(m.kind==='ready'){if(pending){const p=pending;pending=null;workerUsed=true;worker.postMessage(p.message,p.transfer);}else{$('calculate').disabled=false;status('Ready');}}
 if(m.kind==='error'){pending=null;fail(m.message);}
 if(m.kind==='drawing'){
  busy=false;drawing=m.value.paths;position=0;playing=false;$('play').textContent='Play';
  curves=drawing.map(p=>Array.from({length:513},(_,i)=>point(p,i/512)));
  let minx=Infinity,maxx=-Infinity,miny=Infinity,maxy=-Infinity;
  for(const p of drawing)for(const [x,y] of p.preview){minx=Math.min(minx,x);maxx=Math.max(maxx,x);miny=Math.min(miny,y);maxy=Math.max(maxy,y);}
  bounds=[minx,maxx,miny,maxy];const n=drawing.reduce((s,p)=>s+p.coeff.length,0);
  $('stats').textContent=drawing.length+' paths · '+n+' coefficients · '+Math.round(n*12/1024)+' KiB coefficient payload'+(m.value.limited?' · limit reached':'');
  $('equation').replaceChildren(new Option('Instructions',0),...drawing.map((p,i)=>new Option('Path '+(i+1)+' ('+p.name+')',i+1)),new Option('Master Pen',drawing.length+1));
  enabled(true);$('calculate').disabled=false;status('Ready');requestEquation();render();
 }
 if(m.kind==='equation'&&m.id===req){full=m.value;$('output').value=full.slice(0,4000)+(full.length>4000?'\n[Preview truncated; copy includes full equation]':'');$('copy').disabled=false;}
 if(m.kind==='export'){const url=URL.createObjectURL(new Blob([m.value],{type:'text/html'}));const a=document.createElement('a');a.href=url;a.download='fourier-desmos.html';a.click();setTimeout(()=>URL.revokeObjectURL(url),1000);status('Desmos HTML exported.');}
};
function point(p,t){let x=0,y=0;for(const [f,r,a] of p.coeff){const q=2*Math.PI*f*t+a;x+=r*Math.cos(q);y+=r*Math.sin(q);}return [x,y];}
function project([x,y]){const [a,b,c,d]=bounds,s=.9*Math.min(canvas.width/Math.max(b-a,.01),canvas.height/Math.max(d-c,.01));return [canvas.width/2+(x-(a+b)/2)*s,canvas.height/2-(y-(c+d)/2)*s];}
function line(points,color,width=1.5){if(points.length<2)return;ctx.beginPath();for(let i=0;i<points.length;i++){const [x,y]=project(points[i]);if(i===0)ctx.moveTo(x,y);else ctx.lineTo(x,y);}ctx.strokeStyle=color;ctx.lineWidth=width;ctx.stroke();}
function render(){
 ctx.fillStyle='#fff';ctx.fillRect(0,0,canvas.width,canvas.height);if(!bounds||!drawing.length)return;
 if($('original').checked)for(const p of drawing)line(p.preview,'#d1d5db');
 const i=Math.min(drawing.length-1,Math.floor(position)),t=Math.min(1,position-i);
 for(let j=0;j<i;j++)line(curves[j],$('ink').checked?drawing[j].color:'#2563eb',1.6);
 line([...curves[i].slice(0,Math.floor(t*512)+1),point(drawing[i],t)],$('ink').checked?drawing[i].color:'#2563eb',1.6);
 if($('circles').checked&&position<drawing.length){let x=0,y=0;for(const [f,r,a] of drawing[i].coeff){const q=2*Math.PI*f*t+a, p=project([x,y]);const nx=x+r*Math.cos(q),ny=y+r*Math.sin(q),e=project([nx,ny]);const edge=project([x+r,y]);ctx.beginPath();ctx.arc(p[0],p[1],Math.abs(edge[0]-p[0]),0,Math.PI*2);ctx.strokeStyle='#49647b';ctx.lineWidth=.7;ctx.stroke();ctx.beginPath();ctx.moveTo(...p);ctx.lineTo(...e);ctx.strokeStyle='#fbbf24';ctx.stroke();x=nx;y=ny;}const p=project([x,y]);ctx.beginPath();ctx.arc(...p,3,0,Math.PI*2);ctx.fillStyle='#f87171';ctx.fill();}
}
function frame(now,id){if(!playing||id!==animationId)return;position=Math.min(drawing.length,position+Math.min((now-last)/1000,.1)*Number($('speed').value));last=now;render();if(position<drawing.length)requestAnimationFrame(next=>frame(next,id));else{playing=false;$('play').textContent='Play';}}
$('play').onclick=()=>{if(playing){playing=false;animationId++;$('play').textContent='Play';return;}if(position>=drawing.length)position=0;playing=true;last=performance.now();$('play').textContent='Pause';const id=++animationId;requestAnimationFrame(now=>frame(now,id));};
$('reset').onclick=()=>{playing=false;animationId++;position=0;$('play').textContent='Play';render();};
$('finish').onclick=()=>{playing=false;animationId++;position=drawing.length;$('play').textContent='Play';render();};
for(const id of ['circles','original','ink'])$(id).onchange=render;
$('mode').onchange=()=>{const image=$('mode').value==='image';$('imageGroup').hidden=!image;$('colorGroup').hidden=!image;$('textGroup').hidden=image;$('detail').value=image?300:150;$('detailValue').value=$('detail').value;};
$('trace').onchange=()=>$('sensitivityGroup').hidden=$('trace').value!=='color';
for(const id of ['detail','cap','sensitivity'])$(id).oninput=()=>$(id+'Value').value=$(id).value;
$('calculate').onclick=async()=>{
 if(busy)return;busy=true;playing=false;animationId++;req++;drawing=[];curves=[];bounds=null;render();enabled(false);full='';$('output').value='';$('calculate').disabled=true;status('Calculating in Rust…');
 try{
  const mode=$('mode').value,detail=Number($('detail').value),cap=Number($('cap').value);
  if(mode==='image'){const f=$('file').files[0];if(!f)throw Error('Choose an image');if(f.size>16*1024*1024)throw Error('Maximum file size 16 MiB');const bytes=await f.arrayBuffer();sendCalculation({kind:'calculate',mode,detail,cap,bytes,color:$('trace').value==='color',sensitivity:Number($('sensitivity').value)},[bytes]);}
  else{
   const text=$('text').value.trim().replaceAll('[heart]','♥').replaceAll('<3','♥').replaceAll('[square]','■').replaceAll('[star]','★').replaceAll('[note]','♫');if(!text)throw Error('Enter some text');
   const c=document.createElement('canvas'),g=c.getContext('2d',{willReadFrequently:true});g.font='100px Arial, sans-serif';const width=g.measureText(text).width;const font=Math.min(100,3600/Math.max(width,1)*100);g.font=font+'px Arial, sans-serif';c.width=Math.ceil(g.measureText(text).width)+32;c.height=Math.ceil(font*1.7)+32;g.fillStyle='white';g.fillRect(0,0,c.width,c.height);g.font=font+'px Arial, sans-serif';g.textBaseline='middle';g.fillStyle='black';g.fillText(text,16,c.height/2);const rgba=g.getImageData(0,0,c.width,c.height).data;const gray=new Uint8Array(c.width*c.height);for(let i=0;i<gray.length;i++)gray[i]=rgba[i*4];sendCalculation({kind:'calculate',mode,detail,cap,w:c.width,h:c.height,bytes:gray.buffer},[gray.buffer]);
  }
 }catch(e){fail(e);}
};
$('equation').onchange=requestEquation;
$('copy').onclick=async()=>{try{await navigator.clipboard.writeText(full);status('Copied complete equation');}catch(e){status('Clipboard blocked: '+e.message);}};
$('export').onclick=()=>worker.postMessage({kind:'export'});

startWorker();
