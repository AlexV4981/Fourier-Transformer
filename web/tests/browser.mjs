import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { resolve, extname } from 'node:path';
const root=resolve('web/site');
const mime={'.html':'text/html','.js':'text/javascript','.css':'text/css','.wasm':'application/wasm','.json':'application/json'};
const server=createServer(async(req,res)=>{
 try{const url=new URL(req.url,'http://localhost');const file=resolve(root,'.'+(url.pathname==='/'?'/index.html':url.pathname));
 if(!file.startsWith(root+'/')){res.writeHead(403).end();return;}
 const data=await readFile(file);res.writeHead(200,{'Content-Type':mime[extname(file)]||'application/octet-stream'});res.end(data);
 }catch{res.writeHead(404).end();}
});
await new Promise(r=>server.listen(0,'127.0.0.1',r));
let browser,page;
try{
 browser=await chromium.launch({headless:true});
 const context=await browser.newContext({acceptDownloads:true,permissions:['clipboard-read','clipboard-write']});
 page=await context.newPage();const errors=[];
 page.on('console',m=>{if(m.type()==='error')console.log('browser console:',m.text());});
 page.on('pageerror',e=>errors.push(e.message));
 await page.goto('http://127.0.0.1:'+server.address().port);
 await page.waitForFunction(()=>document.querySelector('#status').textContent==='Ready',{},{timeout:60000});
 async function calc(){
   const start=Date.now();
   await page.click('#calculate');
   await page.waitForFunction(()=>document.querySelector('#status').textContent==='Ready'||!document.querySelector('#calculate').disabled,{},{timeout:120000});
   assert.equal(await page.locator('#status').textContent(),'Ready');
   const stats=await page.locator('#stats').textContent();
   const match=stats.match(/(\d+) paths · (\d+) coefficients/);assert.ok(match,stats);
   assert.ok(+match[1]>0&&+match[1]<=200&&+match[2]>0&&+match[2]<=12000,stats);
   console.log('verified:',stats,Date.now()-start+' ms');
   return stats;
 }
 await calc();
 await page.click('#finish');
 await page.selectOption('#equation','1');
 await page.waitForFunction(()=>!document.querySelector('#copy').disabled&&document.querySelector('#output').value.includes('Bounds:'));
 console.log('verified: path preview preserves parameter bounds');
 await page.click('#copy');await page.waitForFunction(()=>document.querySelector('#status').textContent==='Copied complete equation');
 const copied=await page.evaluate(()=>navigator.clipboard.readText());
 assert.ok(copied.includes('Bounds:')&&!copied.includes('[Preview truncated'));
 const preview=await page.locator('#output').inputValue();
 if(preview.includes('[Preview truncated'))assert.ok(copied.length>preview.length);
 console.log('verified: full equation copied');
 await page.selectOption('#equation','0');
 await page.waitForFunction(()=>!document.querySelector('#copy').disabled&&document.querySelector('#output').value.includes('Create slider'));
 await page.selectOption('#equation','1');
 await page.click('#reset');await page.click('#play');await page.waitForTimeout(180);await page.click('#play');
 await page.click('#play');await page.click('#reset');await page.click('#finish');
 await page.selectOption('#mode','image');
 const file=async(type,w,h,pattern)=>{
   const arr=await page.evaluate(async({type,w,h,pattern})=>{
     const c=document.createElement('canvas');c.width=w;c.height=h;const g=c.getContext('2d');
     g.fillStyle='#f5f5f5';g.fillRect(0,0,w,h);g.fillStyle='#ff0000';g.fillRect(w/8,h/8,w/2,h/2);
     g.fillStyle='#004c00';g.fillRect(w/2,h/3,w/3,h/2);g.fillStyle='#1654ff';g.beginPath();g.arc(w*.65,h*.35,h*.2,0,Math.PI*2);g.fill();
     if(pattern==='noise'){for(let y=0;y<h;y+=12)for(let x=0;x<w;x+=12){g.fillStyle='rgb('+((x*73+y*11)%256)+','+((x*19+y*31)%256)+','+((x*17+y*67)%256)+')';g.fillRect(x,y,6,6);}}
     const blob=await new Promise(r=>c.toBlob(r,type,.85));if(!blob||blob.type!==type)throw Error('encoder unavailable '+type);
     return Array.from(new Uint8Array(await blob.arrayBuffer()));
   },{type,w,h,pattern});
   const ext=type.split('/')[1];
   await page.locator('#file').setInputFiles({name:'fixture.'+ext,mimeType:type,buffer:Buffer.from(arr)});
 };
 for(const type of ['image/png','image/jpeg','image/webp']){
   await file(type,320,240,'art');await calc();await page.click('#finish');
   const colorCount=await page.evaluate(()=>{const c=document.querySelector('#plot'),data=c.getContext('2d').getImageData(0,0,c.width,c.height).data;let count=0;for(let i=0;i<data.length;i+=4)if(Math.max(data[i],data[i+1],data[i+2])-Math.min(data[i],data[i+1],data[i+2])>50)count++;return count;});
   assert.ok(colorCount>20,'no colored strokes');
 }
 await page.selectOption('#trace','gray');await calc();await page.selectOption('#trace','color');
 for(const [id,value] of [['detail','500'],['cap','200'],['sensitivity','90']]){
   await page.locator('#'+id).evaluate((el,v)=>{el.value=v;el.dispatchEvent(new Event('input',{bubbles:true}));el.dispatchEvent(new Event('change',{bubbles:true}));},value);
   assert.equal(await page.locator('#'+id).inputValue(),value);
 }
 await file('image/webp',800,600,'noise');
 for(let i=0;i<3;i++)await calc();
 await page.click('#finish');
 const downloadPromise=page.waitForEvent('download');await page.click('#export');const download=await downloadPromise;
 const html=await readFile(await download.path(),'utf8');assert.ok(html.includes('parametricDomain')&&html.includes('pen_0'));
 await page.locator('#file').setInputFiles({name:'broken.webp',mimeType:'image/webp',buffer:Buffer.from('not an image')});
 await page.click('#calculate');await page.waitForFunction(()=>!document.querySelector('#calculate').disabled);
 assert.notEqual(await page.locator('#status').textContent(),'Ready');
 await file('image/png',2001,2000,'art');await page.click('#calculate');await page.waitForFunction(()=>!document.querySelector('#calculate').disabled,{},{timeout:120000});
 assert.match(await page.locator('#status').textContent(),/4 MP/);
 await file('image/png',400,300,'art');await calc();await page.click('#finish');
 assert.deepEqual(errors,[]);await page.screenshot({path:'browser-verified.png'});
 console.log('Browser regression suite passed');
}catch(e){
 if(page){console.error('Failure state:',await page.locator('#status').textContent().catch(()=>''),await page.locator('#stats').textContent().catch(()=>''));await page.screenshot({path:'browser-failure.png',fullPage:true}).catch(()=>{});}
 throw e;
}finally{if(browser)await browser.close();await new Promise(r=>server.close(r));}
