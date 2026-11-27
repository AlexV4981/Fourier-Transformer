use std::{io::{self, Write}, path::Path};
use rustfft::{FftPlanner, num_complex::Complex32 as C};
use image::{GrayImage, Luma};
use ttf_parser::OutlineBuilder;

pub const MAX_PATHS: usize = 200;
pub const MAX_COEFFICIENTS: usize = 12_000;
pub const MAX_PIXELS: u64 = 4_000_000;
pub type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Copy, Debug)]
pub struct Coeff { pub freq: i32, pub amp: f32, pub phase: f32 }
pub struct DrawingPath { pub name: String, pub preview: Vec<C>, pub coeff: Vec<Coeff> }
pub struct Drawing { pub paths: Vec<DrawingPath>, pub limited: bool }
impl Drawing {
    pub fn new() -> Self { Self { paths: Vec::new(), limited: false } }
    pub fn add(&mut self, name: String, points: &[C], n: usize, detail: usize, planner: &mut FftPlanner<f32>) {
        let used: usize = self.paths.iter().map(|p| p.coeff.len()).sum();
        let cap = detail.clamp(1,500).min(MAX_COEFFICIENTS.saturating_sub(used));
        if cap == 0 || self.paths.len() >= MAX_PATHS { self.limited = true; return; }
        if points.len() < 2 { return; }
        let mut samples = resample(points, n);
        let preview = samples.iter().step_by(4).copied().collect();
        let fft = planner.plan_fft_forward(n);
        fft.process(&mut samples);
        let mut coeff: Vec<_> = samples.iter().enumerate().filter_map(|(i,c)| {
            let v = *c / n as f32;
            let amp = v.norm();
            (amp >= 0.001).then_some(Coeff { freq: if i < (n+1)/2 { i as i32 } else { i as i32 - n as i32 }, amp, phase: v.arg() })
        }).collect();
        coeff.sort_unstable_by(|a,b| b.amp.total_cmp(&a.amp));
        if cap < detail && coeff.len() > cap { self.limited = true; }
        coeff.truncate(cap);
        coeff.shrink_to_fit();
        self.paths.push(DrawingPath { name, preview, coeff });
    }
}
pub fn resample(points: &[C], n: usize) -> Vec<C> {
    if points.is_empty() || n == 0 { return vec![]; }
    let total: f32 = points.windows(2).map(|p| (p[1]-p[0]).norm()).sum();
    if total <= f32::EPSILON { return vec![points[0]; n]; }
    let mut j=0; let mut start=0.; let mut len=(points[1]-points[0]).norm();
    (0..n).map(|i| {
        let t = total * i as f32 / n as f32;
        while j+2 < points.len() && (start+len <= t || len == 0.) {
            start += len; j+=1; len=(points[j+1]-points[j]).norm();
        }
        let u = if len > 0. { ((t-start)/len).clamp(0.,1.) } else { 0. };
        points[j]+(points[j+1]-points[j])*u
    }).collect()
}
struct Outline { paths: Vec<Vec<C>>, current: Vec<C> }
impl Outline {
    fn last(&self)->C { self.current.last().copied().unwrap_or_default() }
    fn flush(&mut self) { if self.current.len()>1 { self.paths.push(std::mem::take(&mut self.current)); } else { self.current.clear(); } }
}
impl OutlineBuilder for Outline {
    fn move_to(&mut self,x:f32,y:f32){self.flush();self.current.push(C::new(x,y));}
    fn line_to(&mut self,x:f32,y:f32){self.current.push(C::new(x,y));}
    fn quad_to(&mut self,x1:f32,y1:f32,x:f32,y:f32){
        let a=self.last();let b=C::new(x1,y1);let c=C::new(x,y);
        for i in 1..=12 {let t=i as f32/12.;let s=1.-t;self.current.push(a*s*s+b*2.*s*t+c*t*t);}
    }
    fn curve_to(&mut self,x1:f32,y1:f32,x2:f32,y2:f32,x:f32,y:f32){
        let a=self.last();let b=C::new(x1,y1);let c=C::new(x2,y2);let d=C::new(x,y);
        for i in 1..=16 {let t=i as f32/16.;let s=1.-t;self.current.push(a*s*s*s+b*3.*s*s*t+c*3.*s*t*t+d*t*t*t);}
    }
    fn close(&mut self){if let Some(p)=self.current.first().copied(){self.current.push(p);}self.flush();}
}
pub fn text_drawing(input:&str, detail:usize)->Result<Drawing>{
    let text=input.trim().replace("[heart]","♥").replace("<3","♥").replace("[square]","■").replace("[star]","★").replace("[note]","♫");
    if text.chars().count()>200 {return Err("Text limit: 200 characters".into());}
    let mut db=fontdb::Database::new();db.load_system_fonts();
    let preferred=db.query(&fontdb::Query{families:&[fontdb::Family::Name("Arial"),fontdb::Family::SansSerif],..Default::default()});
    let mut ids:Vec<_>=db.faces().map(|f|f.id).collect();
    if let Some(id)=preferred {ids.retain(|i|*i!=id);ids.insert(0,id);}
    let mut drawing=Drawing::new();let mut planner=FftPlanner::new();let mut x=0.;
    for ch in text.chars(){
        if ch.is_whitespace(){x+=1.5;continue;}
        let mut glyph=None;
        for id in &ids {
            let found=db.with_face_data(*id,|bytes,index|{
                let face=ttf_parser::Face::parse(bytes,index).ok()?;
                let g=face.glyph_index(ch)?;
                let mut out=Outline{paths:vec![],current:vec![]};
                let rect=face.outline_glyph(g,&mut out)?;out.flush();
                Some((out.paths,rect,2./face.units_per_em() as f32))
            }).flatten();
            if found.is_some(){glyph=found;break;}
        }
        let (paths,r,scale)=glyph.ok_or_else(||format!("No installed outline font for {ch}"))?;
        let width=(r.x_max as f32-r.x_min as f32)*scale;
        let cy=(r.y_max as f32+r.y_min as f32)/2.;
        for mut p in paths {
            for v in &mut p {v.re=(v.re-r.x_min as f32)*scale+x;v.im=(v.im-cy)*scale;}
            drawing.add(ch.to_string(),&p,1500,detail,&mut planner);
        }
        x+=width+(width*0.2).max(0.3);
        if drawing.limited {break;}
    }
    if drawing.paths.is_empty(){return Err("No outlines found".into());} Ok(drawing)
}
pub fn image_drawing(path:&Path, detail:usize, max_contours:usize)->Result<Drawing>{
    let open=|| image::ImageReader::open(path).map_err(|e|e.to_string())?.with_guessed_format().map_err(|e|e.to_string());
    let (w,h)=open()?.into_dimensions().map_err(|e|e.to_string())?;
    if w==0 || h==0 || u64::from(w)*u64::from(h)>MAX_PIXELS {return Err("Image exceeds 4 megapixels; resize first.".into());}
    let mut reader=open()?;let mut limits=image::Limits::default();
    limits.max_image_width=Some(w);limits.max_image_height=Some(h);limits.max_alloc=Some(64*1024*1024);reader.limits(limits);
    let gray=reader.decode().map_err(|e|e.to_string())?.into_luma8();
    // Separable 3x3 binomial blur, followed by local Gaussian threshold.
    let blurred=imageproc::filter::gaussian_blur_f32(&gray,0.85);drop(gray);
    let mut mask=GrayImage::new(w,h);
    let weights=[1u32,10,45,120,210,252,210,120,45,10,1];
    // Two-pass local mean, with u16 intermediate to avoid large float buffers.
    let mut horizontal=vec![0u16;(w*h) as usize];
    for y in 0..h {for x in 0..w {
        let sum:u32=weights.iter().enumerate().map(|(i,k)|{
            let xx=(x as i64+i as i64-5).clamp(0,w as i64-1) as u32;
            *k*u32::from(blurred.get_pixel(xx,y)[0])
        }).sum();horizontal[(y*w+x) as usize]=((sum+32)/64) as u16;
    }}
    for y in 0..h {for x in 0..w {
        let sum:u32=weights.iter().enumerate().map(|(i,k)|{
            let yy=(y as i64+i as i64-5).clamp(0,h as i64-1) as u32;
            *k*u32::from(horizontal[(yy*w+x) as usize])
        }).sum();
        let mean=sum as f32/16384.;
        mask.put_pixel(x,y,Luma([if (blurred.get_pixel(x,y)[0] as f32)<mean-2. {255}else{0}]));
    }}
    drop(horizontal);drop(blurred);
    let mut contours=imageproc::contours::find_contours::<i32>(&mask);drop(mask);
    contours.retain(|c|c.points.len()>8);
    contours.sort_unstable_by_key(|c|std::cmp::Reverse(c.points.len()));
    let cap=max_contours.clamp(1,MAX_PATHS);
    let mut drawing=Drawing::new(); drawing.limited=contours.len()>cap;
    contours.truncate(cap);
    let scale=10./w.max(h) as f32; let mut planner=FftPlanner::new();
    for (i,c) in contours.into_iter().enumerate(){
        let mut p:Vec<_>=c.points.into_iter().map(|v|C::new((v.x as f32-w as f32/2.)*scale,(h as f32/2.-v.y as f32)*scale)).collect();
        if let Some(first)=p.first().copied(){p.push(first);}
        drawing.add(format!("Line {}",i+1),&p,400,detail,&mut planner);
    }
    if drawing.paths.is_empty(){return Err("No outlines found".into());}Ok(drawing)
}
pub fn axis<W:Write>(w:&mut W,p:&DrawingPath,var:&str,y:bool)->io::Result<()>{
    if p.coeff.is_empty(){return write!(w,"0");}
    for (i,c) in p.coeff.iter().enumerate(){
        if i>0 {write!(w,"+")?;}
        write!(w,"{:.3}\\{}(2\\pi\\cdot{}\\cdot{}+{:.3})",c.amp,if y{"sin"}else{"cos"},c.freq,var,c.phase)?;
    } Ok(())
}
pub fn equation<W:Write>(w:&mut W,p:&DrawingPath)->io::Result<()>{
    write!(w,"(")?;axis(w,p,"t",false)?;write!(w,",")?;axis(w,p,"t",true)?;write!(w,")")
}
pub fn master<W:Write>(w:&mut W,d:&Drawing)->io::Result<()>{
    write!(w,"(")?;
    for y in [false,true] {
        if y{write!(w,",")?;}write!(w,"\\{{")?;
        for (i,p) in d.paths.iter().enumerate(){
            if i>0{write!(w,",")?;}
            write!(w,"{}\\le a{}{}:",i,if i+1==d.paths.len(){"\\le "}else{"<"},i+1)?;
            axis(w,p,&format!("(a-{i})"),y)?;
        }
        write!(w,"\\}}")?;
    }write!(w,")")
}
pub fn manual(d:&Drawing,index:usize)->String{
    let mut b=Vec::new();
    if index==0 {return format!("Create slider a=0, bounds 0 to {}. Copy each path into Desmos and set its t bounds as shown. Paste Master Pen separately.",d.paths.len());}
    if index>d.paths.len(){master(&mut b,d).unwrap();}
    else{let i=index-1;equation(&mut b,&d.paths[i]).unwrap();write!(b,"\n\nBounds: 0 <= t <= \\min(1,\\max(0,a-{i}))").unwrap();}
    String::from_utf8(b).unwrap()
}
pub fn export_html<W:Write>(w:&mut W,d:&Drawing)->io::Result<()>{
    writeln!(w,"<!doctype html><meta charset='utf-8'><title>Fourier drawing</title><style>html,body,#calculator{{height:100%;margin:0}}</style><div id='calculator'></div><script src='https://www.desmos.com/api/v1.8/calculator.js?apiKey=dcb31709b452b1cf9dc26972add0fda6'></script><script>const calc=Desmos.GraphingCalculator(document.getElementById('calculator'));")?;
    writeln!(w,"calc.setExpression({{id:'a',latex:'a=0',sliderBounds:{{min:0,max:{},step:0.01}}}});",d.paths.len())?;
    for (i,p) in d.paths.iter().enumerate(){
        let mut b=Vec::new();equation(&mut b,p)?;
        let json=serde_json::json!({"id":format!("path_{i}"),"latex":String::from_utf8(b).unwrap(),"parametricDomain":{"min":"0","max":format!("\\min(1,\\max(0,a-{i}))")},"color":"#2d70b3"});
        writeln!(w,"calc.setExpression({json});")?;
        // Each pen point is restricted to its active path; no giant master expression.
        let mut b=Vec::new();write!(b,"(")?;axis(&mut b,p,&format!("(a-{i})"),false)?;write!(b,",")?;axis(&mut b,p,&format!("(a-{i})"),true)?;
        write!(b,")\\{{{i}\\le a{}{}\\}}",if i+1==d.paths.len(){"\\le "}else{"<"},i+1)?;
        let json=serde_json::json!({"id":format!("pen_{i}"),"latex":String::from_utf8(b).unwrap(),"color":"#c74440"});
        writeln!(w,"calc.setExpression({json});")?;
    }
    writeln!(w,"</script>")
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test] fn circle_fft(){
        let points:Vec<_>=(0..=128).map(|i|C::from_polar(1.,std::f32::consts::TAU*i as f32/128.)).collect();
        let mut d=Drawing::new();d.add("circle".into(),&points,128,20,&mut FftPlanner::new());
        assert_eq!(d.paths[0].coeff[0].freq,1);assert!((d.paths[0].coeff[0].amp-1.).abs()<0.001);
    }
    #[test] fn repeated_points(){let p=[C::new(1.,2.);3];assert_eq!(resample(&p,10),vec![p[0];10]);}
    #[test] fn export(){let mut d=Drawing::new();d.add("test".into(),&[C::new(0.,0.),C::new(1.,0.),C::new(0.,0.)],32,10,&mut FftPlanner::new());let mut b=vec![];export_html(&mut b,&d).unwrap();assert!(String::from_utf8(b).unwrap().contains("parametricDomain"));}
}
