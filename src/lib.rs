use std::{io::{self, Write}, path::Path};
use rustfft::{FftPlanner, num_complex::Complex32 as C};
use image::{GrayImage, Luma};
#[cfg(not(target_arch = "wasm32"))]
use ttf_parser::OutlineBuilder;

pub const MAX_PATHS: usize = 200;
pub const MAX_COEFFICIENTS: usize = 12_000;
pub const MAX_PIXELS: u64 = 4_000_000;
pub type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Copy, Debug)]
pub struct Coeff { pub freq: i32, pub amp: f32, pub phase: f32 }
pub struct DrawingPath { pub name: String, pub preview: Vec<C>, pub coeff: Vec<Coeff>, pub color: [u8;3] }
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
        self.paths.push(DrawingPath { name, preview, coeff, color: [45,112,179] });
    }
}
pub fn resample(points: &[C], n: usize) -> Vec<C> {
    if points.is_empty() || n == 0 { return vec![]; }
    if points.len() == 1 { return vec![points[0]; n]; }
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
#[cfg(not(target_arch = "wasm32"))]
struct Outline { paths: Vec<Vec<C>>, current: Vec<C> }
#[cfg(not(target_arch = "wasm32"))]
impl Outline {
    fn last(&self)->C { self.current.last().copied().unwrap_or_default() }
    fn flush(&mut self) { if self.current.len()>1 { self.paths.push(std::mem::take(&mut self.current)); } else { self.current.clear(); } }
}
#[cfg(not(target_arch = "wasm32"))]
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
#[cfg(not(target_arch = "wasm32"))]
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
pub fn image_drawing(path:&Path, detail:usize, max_contours:usize, color:bool, sensitivity:u8)->Result<Drawing>{
    let open=|| image::ImageReader::open(path).map_err(|e|e.to_string())?.with_guessed_format().map_err(|e|e.to_string());
    let (w,h)=open()?.into_dimensions().map_err(|e|e.to_string())?;
    if w==0 || h==0 || u64::from(w)*u64::from(h)>MAX_PIXELS {return Err("Image exceeds 4 megapixels; resize first.".into());}
    let mut reader=open()?;let mut limits=image::Limits::default();
    limits.max_image_width=Some(w);limits.max_image_height=Some(h);limits.max_alloc=Some(64*1024*1024);reader.limits(limits);
    decoded_drawing(reader.decode().map_err(|e|e.to_string())?, detail, max_contours, color, sensitivity)
}

// White compositing prevents invisible RGB under transparent pixels from becoming edges.
pub fn decoded_drawing(input:image::DynamicImage, detail:usize, cap:usize, color:bool, sensitivity:u8)->Result<Drawing>{
    let (w,h)=(input.width(),input.height());
    if w==0 || h==0 || u64::from(w)*u64::from(h)>MAX_PIXELS {return Err("Image exceeds 4 MP".into());}
    let mut rgb=if input.color().has_alpha(){
        let rgba=input.into_rgba8();
        image::RgbImage::from_fn(w,h,|x,y|{
            let p=rgba.get_pixel(x,y);let a=p[3] as u16;
            image::Rgb(std::array::from_fn(|k|((p[k] as u16*a+255*(255-a)+127)/255) as u8))
        })
    }else{input.into_rgb8()};
    if !color {return gray_drawing(image::DynamicImage::ImageRgb8(rgb).into_luma8(),detail,cap,false);}
    // Bound the gradient and contour stage independently of decoded image dimensions.
    const TRACE_PIXELS:f64=1_500_000.;
    if w as f64*h as f64>TRACE_PIXELS{
        let s=(TRACE_PIXELS/(w as f64*h as f64)).sqrt();
        rgb=image::imageops::resize(&rgb,((w as f64*s) as u32).max(1),((h as f64*s) as u32).max(1),image::imageops::FilterType::Triangle);
    }
    color_drawing(&rgb,detail,cap,sensitivity)
}
/// Max-channel Sobel gradient, with directional non-maximum suppression.
/// Captures chromatic boundaries even when grayscale luminance is identical.
pub fn color_drawing(rgb:&image::RgbImage,detail:usize,cap:usize,sensitivity:u8)->Result<Drawing>{
    let (w,h)=rgb.dimensions();
    let n=u64::from(w)*u64::from(h);
    if w<3||h<3||n>1_500_000{return Err("Color tracing requires 3x3 to 1.5 MP after scaling".into());}
    let stride=w as usize;let mut strength=vec![0u16;n as usize];let mut direction=vec![0u8;n as usize];
    let raw=rgb.as_raw();
    for y in 1..h as usize-1 {for x in 1..stride-1{
        let i=y*stride+x;
        let mut best=(0i32,0i32,0i32);
        for k in 0..3{
            let p=|offset:isize|raw[((i as isize+offset) as usize)*3+k] as i32;
            let s=stride as isize;
            let gx=p(-s+1)+2*p(1)+p(s+1)-p(-s-1)-2*p(-1)-p(s-1);
            let gy=p(s-1)+2*p(s)+p(s+1)-p(-s-1)-2*p(-s)-p(-s+1);
            let mag=gx.abs()+gy.abs();
            if mag>best.0 {best=(mag,gx,gy);}
        }
        strength[i]=best.0 as u16;
        let (a,b)=(best.1.abs(),best.2.abs());
        direction[i]=if b*2<a{0}else if a*2<b{1}else if best.1.signum()==best.2.signum(){2}else{3};
    }}
    let mut mask=GrayImage::new(w,h);
    let threshold=320u16-2*sensitivity.clamp(1,100) as u16;
    for y in 1..h as usize-1{for x in 1..stride-1{
        let i=y*stride+x;let v=strength[i];
        if v<threshold{continue;}
        let step=match direction[i]{0=>1,1=>stride,2=>stride+1,_=>stride-1};
        if v>=strength[i-step]&&v>strength[i+step]{mask.as_mut()[i]=255;}
    }}
    drop(strength);drop(direction);
    trace_colored(mask,rgb,detail,cap)
}
fn stroke_color(rgb:&image::RgbImage,pts:&[imageproc::point::Point<i32>])->[u8;3]{
    let mut sum=[0u64;3];let mut count=0u64;
    for p in pts.iter().step_by((pts.len()/64).max(1)){
        let mut best=[0u8;3];let mut score=-1i32;
        for dy in -1..=1{for dx in -1..=1{
            let x=(p.x+dx).clamp(0,rgb.width() as i32-1) as u32;
            let y=(p.y+dy).clamp(0,rgb.height() as i32-1) as u32;
            let c=rgb.get_pixel(x,y).0;
            let hi=*c.iter().max().unwrap() as i32;let lo=*c.iter().min().unwrap() as i32;
            let s=2*(hi-lo)+(255-hi)/2;
            if s>score{score=s;best=c;}
        }}
        for k in 0..3{sum[k]+=best[k] as u64;}count+=1;
    }
    if count==0{[45,112,179]}else{std::array::from_fn(|k|(sum[k]/count) as u8)}
}
fn trace_colored(mask:GrayImage,rgb:&image::RgbImage,detail:usize,cap:usize)->Result<Drawing>{
    let (w,h)=mask.dimensions();
    let mut contours=imageproc::contours::find_contours::<i32>(&mask);drop(mask);
    contours.retain(|c|c.points.len()>8);
    contours.sort_unstable_by_key(|c|std::cmp::Reverse(c.points.len()));
    let cap=cap.clamp(1,MAX_PATHS);let mut d=Drawing::new();d.limited=contours.len()>cap;
    contours.truncate(cap);
    let total=contours.len();let scale=10./w.max(h) as f32;let mut planner=FftPlanner::new();
    let weights:Vec<f32>=contours.iter().map(|c|(c.points.len() as f32).sqrt()).collect();
    let mut remaining_weight: f32=weights.iter().sum();
    for (i,c) in contours.into_iter().enumerate(){
        let used:usize=d.paths.iter().map(|p|p.coeff.len()).sum();
        let left=MAX_COEFFICIENTS.saturating_sub(used);
        // Reserve at least two terms for every remaining outline, and distribute detail by length.
        let reserve=2*(total-i-1);
        let share=((left.saturating_sub(2*(total-i)) as f32*weights[i]/remaining_weight.max(1.)) as usize+2).min(left.saturating_sub(reserve)).min(detail.clamp(1,500));
        remaining_weight-=weights[i];
        if share<detail{d.limited=true;}
        let color=stroke_color(rgb,&c.points);
        let mut p:Vec<_>=c.points.into_iter().map(|v|C::new((v.x as f32-w as f32/2.)*scale,(h as f32/2.-v.y as f32)*scale)).collect();
        if let Some(first)=p.first().copied(){p.push(first);}
        let n=if p.len()>600{1024}else if p.len()>200{512}else{256};
        let before=d.paths.len();d.add(format!("Color line {}",i+1),&p,n,share,&mut planner);
        if d.paths.len()>before{d.paths.last_mut().unwrap().color=color;}
    }
    if d.paths.is_empty(){return Err("No color edges found; increase sensitivity or try grayscale.".into());}Ok(d)
}
pub fn gray_drawing(gray: GrayImage, detail: usize, max_contours: usize, solid: bool) -> Result<Drawing> {
    let (w,h)=gray.dimensions();
    if w==0 || h==0 || u64::from(w)*u64::from(h)>MAX_PIXELS { return Err("Image exceeds 4 megapixels".into()); }
    if solid {
        let mut mask=gray;
        for p in mask.pixels_mut() { p[0]=if p[0]<128 {255}else{0}; }
        return trace_mask(mask, detail, max_contours);
    }
    // Gaussian blur, followed by local Gaussian threshold.
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
    trace_mask(mask,detail,max_contours)
}
fn trace_mask(mask: GrayImage, detail:usize, max_contours:usize)->Result<Drawing>{
    let (w,h)=mask.dimensions();
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
        if c.freq==0{
            let value=c.amp*if y{c.phase.sin()}else{c.phase.cos()};
            // Explicit grouping makes a negative constant safe after a plus sign.
            write!(w,"\\left({:.6}\\right)",value)?;
        }else{
            let sign=if c.phase<0.{"-"}else{"+"};
            write!(w,"{:.6}\\{}\\left({:.6}\\cdot{}{} {:.6}\\right)",c.amp,if y{"sin"}else{"cos"},2.*std::f64::consts::PI*c.freq as f64,var,sign,c.phase.abs())?;
        }
    } Ok(())
}
pub fn equation<W:Write>(w:&mut W,p:&DrawingPath)->io::Result<()>{
    write!(w,"\\left(")?;axis(w,p,"t",false)?;write!(w,",")?;axis(w,p,"t",true)?;write!(w,"\\right)")
}
pub fn master<W:Write>(w:&mut W,d:&Drawing)->io::Result<()>{
    write!(w,"\\left(")?;
    for y in [false,true] {
        if y{write!(w,",")?;}write!(w,"\\left\\{{")?;
        for (i,p) in d.paths.iter().enumerate(){
            if i>0{write!(w,",")?;}
            write!(w,"{}\\le a{}{}:",i,if i+1==d.paths.len(){"\\le "}else{"<"},i+1)?;
            axis(w,p,&format!("\\left(a-{i}\\right)"),y)?;
        }
        write!(w,"\\right\\}}")?;
    }write!(w,"\\right)")
}
/// Clipboard payload is a single expression, with no human-readable instructions.
pub fn paste_expression(d:&Drawing,index:usize)->String{
    if index==0{return "a=0".into();}
    let mut b=Vec::new();
    if index>d.paths.len(){master(&mut b,d).unwrap();}
    else{equation(&mut b,&d.paths[index-1]).unwrap();}
    String::from_utf8(b).unwrap()
}
pub fn path_bound(index:usize)->String{
    format!("\\min\\left(1,\\max\\left(0,a-{}\\right)\\right)",index.saturating_sub(1))
}
pub fn manual(d:&Drawing,index:usize)->String{
    let mut b=Vec::new();
    if index==0 {return format!("Create slider a=0, bounds 0 to {}. Copy each path into Desmos and set its t bounds as shown. Paste Master Pen separately.",d.paths.len());}
    if index>d.paths.len(){master(&mut b,d).unwrap();}
    else{let i=index-1;equation(&mut b,&d.paths[i]).unwrap();write!(b,"\n\nBounds: 0 <= t <= {}",path_bound(index)).unwrap();}
    String::from_utf8(b).unwrap()
}
pub fn export_html<W:Write>(w:&mut W,d:&Drawing)->io::Result<()>{
    writeln!(w,"<!doctype html><meta charset='utf-8'><title>Fourier drawing</title><style>html,body,#calculator{{height:100%;margin:0}}</style><div id='calculator'></div><script src='https://www.desmos.com/api/v1.8/calculator.js?apiKey=dcb31709b452b1cf9dc26972add0fda6'></script><script>const calc=Desmos.GraphingCalculator(document.getElementById('calculator'));")?;
    writeln!(w,"calc.setExpression({{id:'a',latex:'a=0',sliderBounds:{{min:0,max:{},step:0.01}}}});",d.paths.len())?;
    for (i,p) in d.paths.iter().enumerate(){
        let mut b=Vec::new();equation(&mut b,p)?;
        let json=serde_json::json!({"id":format!("path_{i}"),"latex":String::from_utf8(b).unwrap(),"parametricDomain":{"min":"0","max":path_bound(i+1)},"color":format!("#{:02x}{:02x}{:02x}",p.color[0],p.color[1],p.color[2])});
        writeln!(w,"calc.setExpression({json});")?;
        // Each pen point is restricted to its active path; no giant master expression.
        let mut b=Vec::new();write!(b,"\\left(")?;axis(&mut b,p,&format!("\\left(a-{i}\\right)"),false)?;write!(b,",")?;axis(&mut b,p,&format!("\\left(a-{i}\\right)"),true)?;
        write!(b,"\\right)\\left\\{{{i}\\le a{}{}\\right\\}}",if i+1==d.paths.len(){"\\le "}else{"<"},i+1)?;
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
    #[test] fn color_boundary(){
        // These red/green colors have approximately equal Rec.709 luminance.
        let rgb=image::RgbImage::from_fn(96,96,|x,y|if (20..76).contains(&x)&&(20..76).contains(&y){image::Rgb([255,0,0])}else{image::Rgb([0,76,0])});
        let d=color_drawing(&rgb,100,80,65).unwrap();
        assert!(!d.paths.is_empty());assert!(d.paths.iter().map(|p|p.coeff.len()).sum::<usize>()<=MAX_COEFFICIENTS);
        assert!(d.paths.iter().any(|p|p.color[0]>p.color[1]));
    }
    #[test] fn webp_lossless(){
        let rgb=image::RgbImage::from_fn(64,64,|x,y|if (12..52).contains(&x)&&(12..52).contains(&y){image::Rgb([250,10,30])}else{image::Rgb([10,90,40])});
        let mut encoded=Vec::new();
        image::codecs::webp::WebPEncoder::new_lossless(&mut encoded).encode(rgb.as_raw(),64,64,image::ExtendedColorType::Rgb8).unwrap();
        let decoded=image::load_from_memory(&encoded).unwrap();
        assert_eq!(decoded.to_rgb8(),rgb);
        assert!(!decoded_drawing(decoded,100,80,true,65).unwrap().paths.is_empty());
    }
    #[test] fn desmos_clipboard_is_expression_only(){
        let mut d=Drawing::new();
        d.add("test".into(),&[C::new(0.,0.),C::new(1.,0.),C::new(0.,0.)],32,10,&mut FftPlanner::new());
        let s=paste_expression(&d,1);
        assert!(s.starts_with("\\left(")&&s.ends_with("\\right)"));
        assert!(!s.contains("Bounds:")&&!s.contains('\n'));
        assert!(manual(&d,1).contains("Bounds:"));
        assert_eq!(paste_expression(&d,0),"a=0");
        assert_eq!(path_bound(1),"\\min\\left(1,\\max\\left(0,a-0\\right)\\right)");
    }
    #[test] fn repeated_points(){let p=[C::new(1.,2.);3];assert_eq!(resample(&p,10),vec![p[0];10]);}
    #[test] fn export(){let mut d=Drawing::new();d.add("test".into(),&[C::new(0.,0.),C::new(1.,0.),C::new(0.,0.)],32,10,&mut FftPlanner::new());let mut b=vec![];export_html(&mut b,&d).unwrap();assert!(String::from_utf8(b).unwrap().contains("parametricDomain"));}
}
